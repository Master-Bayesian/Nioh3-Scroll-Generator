"""Find selected RIP-relative references, validating candidate instruction boundaries.

This is a bounded opcode search, not a complete cross-reference graph. Raw
sections remain local and results are review evidence, not invocation authority.
"""
import argparse
import bisect
import json
from pathlib import Path
import struct
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path[:0] = [str(ROOT), str(ROOT / 'audit/runtime_deps')]
from capstone import Cs, CS_ARCH_X86, CS_MODE_64
from capstone.x86 import X86_OP_MEM, X86_REG_RIP
from research.native_xrefs import parse_runtime_functions


def find(text, pdata, targets):
    ranges = parse_runtime_functions(pdata)
    starts = [r.begin_rva for r in ranges]
    decoder = Cs(CS_ARCH_X86, CS_MODE_64)
    decoder.detail = True
    candidates = set()
    # MOV/LEA and byte/dword test/compare/store forms, with optional REX.
    for opcode in (0x8B, 0x8D, 0x89, 0x88, 0x8A, 0x80, 0x81, 0x83, 0xC6, 0xC7, 0xF6, 0xF7):
        cursor = 0
        while True:
            offset = text.find(bytes([opcode]), cursor)
            if offset < 0:
                break
            cursor = offset + 1
            if offset + 6 > len(text) or text[offset + 1] & 0xC7 != 5:
                continue
            disp = struct.unpack_from('<i', text, offset + 2)[0]
            # Immediate lengths determine the RIP base for stores/comparisons.
            for immediate in (0, 1, 4):
                if 0x1000 + offset + 6 + immediate + disp in targets:
                    candidates.add(offset + 0x1000)
    decoded, result, unresolved = {}, [], []
    for candidate in sorted(candidates):
        index = bisect.bisect_right(starts, candidate) - 1
        if index < 0 or candidate >= ranges[index].end_rva:
            continue
        function = ranges[index]
        if function.begin_rva not in decoded:
            raw = text[function.begin_rva - 0x1000:function.end_rva - 0x1000]
            rows = list(decoder.disasm(raw, function.begin_rva))
            if sum(i.size for i in rows) != len(raw):
                unresolved.append({'function_begin': hex(function.begin_rva),
                                   'reason': 'Incomplete containing-range decode'})
                rows = []
            decoded[function.begin_rva] = rows
        for i in decoded[function.begin_rva]:
            if not i.address <= candidate < i.address + i.size:
                continue
            for operand in i.operands:
                if operand.type != X86_OP_MEM or operand.mem.base != X86_REG_RIP:
                    continue
                target = i.address + i.size + operand.mem.disp
                if target in targets:
                    row = {'target_rva': hex(target), 'rva': hex(i.address),
                           'bytes_hex': bytes(i.bytes).hex(), 'mnemonic': i.mnemonic,
                           'operands': i.op_str, 'function_begin': hex(function.begin_rva),
                           'function_end': hex(function.end_rva)}
                    if row not in result:
                        result.append(row)
    return {'schema': 'nioh3-bounded-rip-references/v1', 'references': result,
            'unresolved_ranges': unresolved,
            'limits': ['Selected opcode forms only; no absence or complete call-graph proof.',
                       'Pdata ranges may be fragments, not callable entries.']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--target', action='append', type=lambda v: int(v, 0), required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    directory = ROOT / 'audit/runtime_sections/v2.0.1.0_20260902_title'
    report = find((directory / 'Nioh3_v2.0.1.0.text.bin').read_bytes(),
                  (directory / 'Nioh3_v2.0.1.0.pdata.bin').read_bytes(), set(args.target))
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps({'reference_count': len(report['references']),
                      'functions': sorted({r['function_begin'] for r in report['references']})}))
