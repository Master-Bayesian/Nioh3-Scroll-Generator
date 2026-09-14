"""Bounded exporter behavior on synthetic PE/exception data; no game process."""
from pathlib import Path
import struct
import sys
import pytest
ROOT=Path(__file__).resolve().parents[1]
sys.path.insert(0,str(ROOT/'research/possessed_enemy_capture'))
from dump_augmentation_function_bodies import (REQUESTS, BoundedReader, CaptureError, RuntimeFunction,
                                             Image, Section, read_image, collect, rel32_call_target,
                                             PINNED_LEAF_INTERVALS)
BASE=0x140000000
LEAF_TARGET=0x13684C
LEAF_BODY=bytes.fromhex(
    '4C8B4108448BD2488B014D8BC849C1E1044C03C8EB12488D04C84883C9FF482BCA'
    '4883C0104C03C14D85C07417498BD048D1EA488BCA4803C9443914C872D74C8BC2'
    'EBE4493BC17405443B10730333C0C3488B4008C3'
)


def image_fixture():
    memory={}
    def write(rva,data):
        for i,x in enumerate(data):memory[BASE+rva+i]=x
    def read(a,n):
        assert BASE<=a and a+n<=BASE+0x5000000
        return bytes(memory.get(a+i,0) for i in range(n))
    dos=bytearray(64);dos[:2]=b'MZ';struct.pack_into('<I',dos,60,0x80);write(0,dos)
    h=bytearray(24);h[:4]=b'PE\0\0';struct.pack_into('<HH',h,4,0x8664,3);struct.pack_into('<H',h,20,240);write(0x80,h)
    opt=bytearray(240);struct.pack_into('<H',opt,0,0x20B);struct.pack_into('<I',opt,56,0x5000000)
    struct.pack_into('<I',opt,108,16);struct.pack_into('<II',opt,136,0x4000000,len(REQUESTS)*12);write(0x98,opt)
    secs=[]
    for name,rva,size,flags in [('.text',0x1000,0x3000000,0x60000020),('.pdata',0x4000000,4096,0x40000040),('.xdata',0x4100000,4096,0x40000040)]:
        s=bytearray(40);s[:len(name)]=name.encode();struct.pack_into('<III',s,8,size,rva,size);struct.pack_into('<I',s,36,flags);secs.append(s)
    write(0x188,b''.join(secs))
    pdata=b''.join(struct.pack('<III',x,x+16,0x4100000) for x in sorted(r[2] for r in REQUESTS));write(0x4000000,pdata)
    for call,sig,target,_ in REQUESTS:write(call,bytes.fromhex(sig));write(target,b'\x90'*15+b'\xc3')
    return read,write,memory


def test_exact_functions_not_containing_arbitrary_numbers():
    read,_,_=image_fixture();reader=BoundedReader(read);image=read_image(reader,BASE);r=collect(reader,BASE,image)
    assert len(r['functions'])==8 and r['breakpoints_used']==0 and not r['mechanism_resolved']
    assert all(x['end_rva']-x['begin_rva']==16 for x in r['functions'])
    assert r['read_bytes']<10000
    with pytest.raises(CaptureError,match='BEGIN'):image.exact_function(REQUESTS[0][2]+1)


def test_pinned_leaf_without_pdata_is_exported_by_verified_control_flow_bytes():
    read,write,_=image_fixture();reader=BoundedReader(read);image=read_image(reader,BASE)
    image=Image(image.size,image.sections,image.exception_rva,image.exception_data,
                tuple(f for f in image.functions if f.begin != LEAF_TARGET))
    write(LEAF_TARGET,LEAF_BODY)
    result=collect(reader,BASE,image)
    leaf=next(f for f in result['functions'] if f['begin_rva']==LEAF_TARGET)
    assert leaf['end_rva']==PINNED_LEAF_INTERVALS[LEAF_TARGET][0]
    assert leaf['unwind_rva'] is None
    assert leaf['boundary_source']=='pinned_leaf_complete_control_flow'


def test_pinned_leaf_without_pdata_rejects_changed_bytes():
    read,_,_=image_fixture();reader=BoundedReader(read);image=read_image(reader,BASE)
    image=Image(image.size,image.sections,image.exception_rva,image.exception_data,
                tuple(f for f in image.functions if f.begin != LEAF_TARGET))
    with pytest.raises(CaptureError,match='pinned leaf bytes changed'):
        collect(reader,BASE,image)


@pytest.mark.parametrize('call,sig,target,purpose',REQUESTS)
def test_all_pinned_call_displacements(call,sig,target,purpose):
    assert rel32_call_target(call,bytes.fromhex(sig))==target


@pytest.mark.parametrize('raw',[b'\xe9\0\0\0\0',b'\xe8\0\0',b''])
def test_no_guessing_from_noncall_bytes(raw):
    with pytest.raises(CaptureError):rel32_call_target(100,raw)


def test_modified_caller_signature_rejected():
    read,write,_=image_fixture();reader=BoundedReader(read);image=read_image(reader,BASE)
    write(REQUESTS[0][0],b'\xcc')
    with pytest.raises(CaptureError,match='signature'):collect(reader,BASE,image)


def test_modified_between_two_code_reads_rejected():
    read,_,_=image_fixture();n=0
    def changing(a,size):
        nonlocal n
        b=read(a,size)
        if a==BASE+REQUESTS[0][2]:
            n+=1
            if n==2:return b'\xcc'+b[1:]
        return b
    reader=BoundedReader(changing);image=read_image(reader,BASE)
    with pytest.raises(CaptureError,match='changed'):collect(reader,BASE,image)


def test_no_truncated_function_export():
    im=Image(0x5000000,(Section('.text',0x1000,0x3000000,0x20000000),),0,b'',(RuntimeFunction(0x2000,0x20001,0x3000),))
    with pytest.raises(CaptureError,match='never truncate'):im.exact_function(0x2000)


def test_non_executable_function_rejected():
    im=Image(0x10000,(Section('.rdata',0x1000,0x5000,0),),0,b'',(RuntimeFunction(0x2000,0x2010,0x3000),))
    with pytest.raises(CaptureError,match='executable'):im.exact_function(0x2000)


@pytest.mark.parametrize('rva,data',[
    (0,b'ZZ'),(0x3C,struct.pack('<I',0x20000)),(0x80,b'XX\0\0'),(0x84,b'\x4c\x01'),
    (0x98,b'\x0b\x01'),(0x98+136,struct.pack('<II',0x4000000,13)),
    (0x98+136,struct.pack('<II',0x4ffffff,24)),
])
def test_invalid_pe_bounds_fail_closed(rva,data):
    read,write,_=image_fixture();write(rva,data)
    with pytest.raises(CaptureError):read_image(BoundedReader(read),BASE)


def test_pdata_invalid_sort_rejected():
    read,write,_=image_fixture();write(0x4000000,struct.pack('<III',0x2f00000,0x2f00001,0x4100000))
    with pytest.raises(CaptureError,match='runtime function'):read_image(BoundedReader(read),BASE)


def test_read_budget_and_short_reads_are_not_empty_proofs():
    with pytest.raises(CaptureError,match='budget'):BoundedReader(lambda a,n:b'\0'*n,limit=2)(BASE,4)
    with pytest.raises(CaptureError,match='short'):BoundedReader(lambda a,n:b'')(BASE,4)

from dump_augmentation_function_bodies import parse_pdata_section, from_section_bytes


def test_offline_pdata_padding_is_not_misread_as_function():
    raw=struct.pack('<III',0x1000,0x1100,0x2000)
    assert parse_pdata_section(raw+b'\0'*20)[0]==raw


def test_pdata_zero_hole_not_skipped():
    raw=struct.pack('<III',0x1000,0x1100,0x2000)
    with pytest.raises(CaptureError):parse_pdata_section(raw+b'\0'*12+raw)


def test_offline_wrong_build_fails_before_extraction():
    with pytest.raises(CaptureError,match='SHA-256'):from_section_bytes(b'not game',b'\0'*12)
