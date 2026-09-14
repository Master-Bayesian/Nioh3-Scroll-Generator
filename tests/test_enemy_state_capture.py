import importlib.util
from pathlib import Path
import struct
import pytest

PATH=Path(__file__).resolve().parents[1]/'research/enemy_states_v201/capture_tables.py'
spec=importlib.util.spec_from_file_location('enemy_state_capture',PATH);m=importlib.util.module_from_spec(spec);spec.loader.exec_module(m)

class Memory:
    def __init__(self):self.data=bytearray(0x20000);self.calls=0
    def put(self,address,data):self.data[address:address+len(data)]=data
    def read(self,address,size):
        self.calls+=1
        return bytes(self.data[address:address+size])


def native_table(width=4):
    mem=Memory();ctx=0x1000;store=0x2000;hp=0x3000;entry=0x4000
    c=bytearray(40);struct.pack_into('<Q',c,0,store);struct.pack_into('<Q',c,32,hp);mem.put(ctx,c)
    mem.put(store,struct.pack('<II',0,2)+bytes(range(16)))
    h=bytearray(32);struct.pack_into('<IQQ',h,4,0xffffffff,entry,entry+24);mem.put(hp,h)
    mem.put(entry,struct.pack('<IIIIII',5,0,7,1,0xffffffff,0xffffffff))
    return mem,ctx


@pytest.mark.parametrize('width',[2,4])
def test_hash_lookup_preserves_native_row_mapping(width):
    mem,ctx=native_table(width);t=m.indexed_table(m.BudgetReader(mem),ctx,8,width)
    assert t['mapping']=={5:0,7:1};assert m.row(t,5)==bytes(range(8));assert m.row(t,77) is None


def test_duplicate_hash_key_rejected():
    mem,ctx=native_table();mem.put(0x4008,struct.pack('<II',5,1))
    with pytest.raises(m.CaptureError,match='Duplicate|duplicate'):m.indexed_table(m.BudgetReader(mem),ctx,8,4)


def test_native_out_of_range_row_is_null_not_captured_content():
    mem,ctx=native_table();mem.put(0x4008,struct.pack('<II',7,0xfffffffe))
    t=m.indexed_table(m.BudgetReader(mem),ctx,8,4)
    assert m.row(t,7) is None


def test_table_change_rejected():
    mem,ctx=native_table();original=mem.read
    def read(addr,size):
        b=original(addr,size)
        if addr==0x1000 and mem.calls>1:return b[:-1]+b'\x01'
        return b
    mem.read=read
    with pytest.raises(m.CaptureError,match='changed'):m.indexed_table(m.BudgetReader(mem),ctx,8,4)


@pytest.mark.parametrize('kind',['budget','time','pointer','short','count','stride'])
def test_capture_limits_reject_incomplete_reads(kind):
    mem,ctx=native_table();r=m.BudgetReader(mem)
    if kind=='budget':r.remaining=3
    if kind=='time':r.deadline=0
    if kind=='pointer':ctx=0
    if kind=='short':ctx=len(mem.data)+10
    if kind=='count':mem.put(0x2004,struct.pack('<I',4097))
    if kind=='stride':mem.put(0x3010,struct.pack('<Q',0x4011))
    with pytest.raises(m.CaptureError):m.indexed_table(r,ctx,8,4)


def test_identity_signature_error_stops_before_manager_read():
    mem=Memory();mem.read=lambda address,size: b"\x00"*size
    with pytest.raises(m.CaptureError,match='signature'):m.capture(m.BudgetReader(mem),0x1000)


def test_capture_code_contains_no_target_writes_or_game_invocation():
    s=PATH.read_text()
    for forbidden in ['.WriteProcessMemory(','.CreateRemoteThread(','.SuspendThread(','.DebugActiveProcess(','.TerminateProcess(']:
        assert forbidden not in s
    assert 'PROCESS_VM_WRITE' not in s
    assert 'verified_executable(r)' in s
