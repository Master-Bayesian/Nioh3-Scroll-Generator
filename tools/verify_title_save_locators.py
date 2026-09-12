"""Verify supplied PC v2.01 locator bytes/boundaries; no semantic/live PASS claim."""
from __future__ import annotations
import argparse
from bisect import bisect_right
import hashlib
import json
from pathlib import Path
import re
import shutil
import struct
import subprocess
import tempfile


def decode_instruction_boundaries(
    decoder: str,
    source: Path,
    *,
    function_rva: int,
    temporary_directory: Path,
) -> set[int]:
    """Decode a raw x64 function with GNU objdump or LLVM's Windows tools."""

    executable_name = Path(decoder).name.casefold()
    if executable_name.startswith('llvm-objdump'):
        objcopy_name = 'llvm-objcopy.exe' if Path(decoder).suffix else 'llvm-objcopy'
        objcopy = Path(decoder).with_name(objcopy_name)
        if not objcopy.is_file():
            raise FileNotFoundError(
                f'llvm-objcopy is required beside llvm-objdump: {objcopy}'
            )
        wrapped = temporary_directory / 'function.o'
        subprocess.run(
            [str(objcopy), '-I', 'binary', '-O', 'elf64-x86-64', '-B',
             'i386:x86-64', str(source), str(wrapped)],
            capture_output=True, text=True, check=True, timeout=15,
        )
        command = [
            decoder, '-D', '--section=.data', '--x86-asm-syntax=intel',
            f'--adjust-vma={function_rva}', str(wrapped),
        ]
    else:
        command = [
            decoder, '-D', '-b', 'binary', '-m', 'i386:x86-64', '-Mintel',
            '--insn-width=16', f'--adjust-vma={function_rva}', str(source),
        ]
    result = subprocess.run(
        command, capture_output=True, text=True, check=True, timeout=15
    )
    return {
        int(match.group(1), 16)
        for match in re.finditer(r'^\s*([0-9a-f]+):\s', result.stdout, re.M)
    }


def verify(sections: Path, locators: Path, *, objdump: str | None = None) -> dict:
    model = json.loads(locators.read_text(encoding='utf-8'))
    blobs = {}
    for section in model['section_provenance']:
        raw = (sections / section['filename']).read_bytes()
        if len(raw) != section['size'] or hashlib.sha256(raw).hexdigest().upper() != section['sha256']:
            raise ValueError(f"Wrong static section: {section['name']}")
        blobs[section['name']] = raw
    raw = blobs['.text']; base = model['text_rva']
    pdata = blobs['.pdata']
    ranges = sorted((a,b) for a,b,_ in struct.iter_unpack('<III',pdata[:len(pdata)//12*12])
                    if base <= a < b <= base+len(raw))
    starts = [a for a,_ in ranges]
    decoder = objdump or shutil.which('objdump')
    boundaries = {}
    rows = []
    for name, site in model['sites'].items():
        rva = site['rva']; signature = site['signature_rva']
        index = bisect_right(starts, rva)-1
        if index < 0 or not ranges[index][0] <= rva < ranges[index][1]:
            raise ValueError(f'No runtime function for {name}')
        function = ranges[index]
        if function != (site['unwind_begin'],site['unwind_end']):
            raise ValueError(f'Function boundary mismatch for {name}')
        expected=bytes.fromhex(site['expected_hex']); mask=bytes.fromhex(site['mask_hex'])
        if len(expected)!=len(mask) or any(m not in (0,255) for m in mask):
            raise ValueError(f'Invalid mask for {name}')
        if raw[signature-base:signature-base+len(expected)] != expected:
            raise ValueError(f'Exact provenance bytes changed at {name}')
        pattern=b''.join(re.escape(bytes([b])) if m else b'.' for b,m in zip(expected,mask))
        hits=[match.start()+base for match in re.finditer(pattern,raw,re.DOTALL)]
        if hits != [signature]:
            raise ValueError(f'Non-unique/moved pattern for {name}: {hits[:4]}')
        independent=False
        if decoder:
            if function not in boundaries:
                a,b=function
                with tempfile.TemporaryDirectory(prefix='nioh3-readonly-static-') as tmp:
                    temporary_directory = Path(tmp)
                    source=temporary_directory/'function.bin'
                    source.write_bytes(raw[a-base:b-base])
                    boundaries[function] = decode_instruction_boundaries(
                        decoder,
                        source,
                        function_rva=a,
                        temporary_directory=temporary_directory,
                    )
            if rva not in boundaries[function] or signature not in boundaries[function]:
                raise ValueError(f'Not a decoded instruction boundary: {name}')
            independent=True
        rows.append({'site':name,'rva':hex(rva),'signature_rva':hex(signature),
                     'unique_match':True,'pdata_range':list(map(hex,function)),
                     'independent_objdump_boundary':independent})
    return {'schema':'nioh3.title-save-static-verification/v1','verified_sites':len(rows),
            'objdump':decoder,'sites':rows,'semantic_verification':'manual_control_data_flow_in_RESEARCH_FINDINGS',
            'native_live_acceptance':False,'release':'BLOCK'}


def main() -> int:
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--sections',type=Path,required=True)
    parser.add_argument('--locators',type=Path,default=Path(__file__).resolve().parents[1]/'research/title_save_v201/locators.json')
    parser.add_argument('--objdump')
    parser.add_argument('--output',type=Path)
    args=parser.parse_args()
    result=verify(args.sections,args.locators,objdump=args.objdump)
    text=json.dumps(result,indent=2)+'\n'
    if args.output:
        with args.output.open('x',encoding='utf-8') as stream:stream.write(text)
    else:print(text,end='')
    return 0


if __name__=='__main__':
    raise SystemExit(main())
