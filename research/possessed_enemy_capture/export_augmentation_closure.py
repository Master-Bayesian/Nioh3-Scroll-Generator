"""Export v2.01 unwind-linked code ranges, not a single .pdata fragment.

Offline only. Inputs are pinned .text/.rdata/.pdata section dumps. This replaces
single-RUNTIME_FUNCTION export for this investigation. It does NOT purport to
recover transitive callees, dynamic dispatch or a complete semantic function.
No debugger, target process, game function invocation or target write is used.
"""
from __future__ import annotations

import argparse
from dataclasses import dataclass
import hashlib
import json
from pathlib import Path
import re
import struct
import subprocess
import tempfile

TEXT_RVA = 0x1000
RDATA_RVA = 0x38DE000
PDATA_RVA = 0x4E24000
IMAGE_SIZE = 0x6000000
PINNED = {
    'text': (59624448, 'F8799B5DB54A9CA46F52BCD6C037B2AD9B413DC83A26F1D3F0E61251BFB48023'),
    'rdata': (12392448, 'DF245B4EF1478643845680DB183EA170F448B1EF8FF9710CBD040CB48F4E363D'),
    'pdata': (2828800, '928D0407FFCCFBD081559A2DF00D4995598B60B5C3F1D325D086CB531B09B904'),
}
# All are previously supplied E8 call instructions, not numeric-text discovery.
REQUESTS = (
    (0x2235E04, 'E8F3060000', 0x22364FC),
    (0x1C245C7, 'E8745721FF', 0xE39D40),
    (0x1C2460F, 'E8382251FE', 0x13684C),
    (0x4BC93F, 'E8B40C0000', 0x4BD5F8),
    (0x4BC919, 'E806130000', 0x4BDC24),
    (0x1C24540, 'E87B29FDFF', 0x1BF6EC0),
    (0x223799B, 'E860E99EFF', 0x1C26300),
    (0x22379CC, 'E85FCA9EFF', 0x1C24430),
)
# A genuine leaf has no unwind record. Its complete bounded CFG was separately
# disassembled and verified; no "next .pdata entry" inference is allowed.
LEAF = {0x13684C: (0x1368A2, '627C74A4D65ED70D8F1D6A6D699C6786B23E4A49D5C3EDCF0B81FD134851BF59')}


class ExportError(ValueError):
    pass


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest().upper()


@dataclass(frozen=True, order=True)
class RuntimeRange:
    begin: int
    end: int
    unwind: int


@dataclass(frozen=True)
class Section:
    rva: int
    data: bytes

    def read(self, rva: int, length: int) -> bytes:
        if type(rva) is not int or type(length) is not int or length < 0:
            raise ExportError('invalid read')
        offset = rva - self.rva
        if offset < 0 or offset + length > len(self.data):
            raise ExportError(f'read outside supplied section: {rva:#x}+{length:#x}')
        return self.data[offset:offset+length]


def parse_pdata(data: bytes) -> tuple[RuntimeRange, ...]:
    if not data or len(data) > 4*1024*1024:
        raise ExportError('empty/oversize pdata')
    result = []
    previous_end = 0
    used = 0
    for off in range(0, len(data)-11, 12):
        b, e, u = struct.unpack_from('<III', data, off)
        if b == e == u == 0:
            if any(data[off:]):
                raise ExportError('nonzero data after pdata padding')
            break
        if not 0 < b < e <= IMAGE_SIZE or not 0 < u < IMAGE_SIZE or b < previous_end:
            raise ExportError('invalid/overlapping/unsorted pdata range')
        result.append(RuntimeRange(b, e, u))
        previous_end = e
        used = off+12
    if not result or any(data[used:]):
        raise ExportError('invalid pdata suffix')
    return tuple(result)


class UnwindIndex:
    """Resolve exact chained metadata, including reverse-linked cold fragments.

    Only the common v1/v2 header and slot count are decoded; this is not a stack
    unwinder and makes no claim to execute individual UNWIND_CODE operations.
    """
    def __init__(self, pdata: bytes, rdata: Section):
        self.ranges = parse_pdata(pdata)
        self.by_begin = {x.begin: x for x in self.ranges}
        self.rdata = rdata
        self.parents: dict[int, int | None] = {}
        self.info: dict[int, dict] = {}
        for record in self.ranges:
            header = rdata.read(record.unwind, 4)
            version, flags, count = header[0] & 7, header[0] >> 3, header[2]
            if version not in (1, 2) or flags & ~7 or ((flags & 4) and (flags & 3)):
                raise ExportError(f'unsupported/contradictory unwind header at {record.unwind:#x}')
            size = 4 + 2*((count+1) & ~1)
            parent = None
            if flags & 4:
                chain = RuntimeRange(*struct.unpack('<III', rdata.read(record.unwind+size, 12)))
                if self.by_begin.get(chain.begin) != chain:
                    raise ExportError('chained tuple does not match an exact pdata entry')
                parent = chain.begin
                size += 12
            elif flags & 3:
                size += 4  # handler RVA only; language-specific data is not parsed
            raw = rdata.read(record.unwind, size)
            self.parents[record.begin] = parent
            self.info[record.begin] = {'rva': record.unwind, 'raw_hex': raw.hex().upper(),
                'version': version, 'flags': flags, 'count_of_codes': count, 'parent_begin': parent}
        self.owners: dict[int, int] = {}
        def resolve(begin: int) -> int:
            seen = []
            node = begin
            while node not in self.owners:
                if node in seen or len(seen) >= 64:
                    raise ExportError('cyclic/overlong unwind chain')
                seen.append(node)
                parent = self.parents[node]
                if parent is None:
                    owner = node
                    break
                node = parent
            else:
                owner = self.owners[node]
            for item in seen:
                self.owners[item] = owner
            return owner
        self.groups: dict[int, list[RuntimeRange]] = {}
        for item in self.ranges:
            self.groups.setdefault(resolve(item.begin), []).append(item)

    def group(self, entry: int) -> tuple[RuntimeRange, ...]:
        if entry not in self.by_begin:
            raise ExportError(f'no exact pdata BEGIN at {entry:#x}')
        ranges = tuple(self.groups[self.owners[entry]])
        if len(ranges) > 128 or sum(x.end-x.begin for x in ranges) > 128*1024:
            raise ExportError('unwind group exceeds bounded export')
        return ranges


def call_target(site: int, raw: bytes) -> int:
    if len(raw) != 5 or raw[0] != 0xE8:
        raise ExportError('expected direct E8 instruction')
    return site + 5 + struct.unpack_from('<i', raw, 1)[0]


def check_identity(name: str, raw: bytes) -> None:
    size, sha = PINNED[name]
    if len(raw) != size or digest(raw) != sha:
        raise ExportError(f'{name} size/SHA-256 does not match the supplied PC v2.01 identity')


def build_export(text: bytes, rdata: bytes, pdata: bytes) -> dict:
    for name, raw in (('text', text), ('rdata', rdata), ('pdata', pdata)):
        check_identity(name, raw)
    code = Section(TEXT_RVA, text)
    index = UnwindIndex(pdata, Section(RDATA_RVA, rdata))
    functions = []
    for site, sig, target in REQUESTS:
        call = code.read(site, 5)
        if call.hex().upper() != sig or call_target(site, call) != target:
            raise ExportError(f'call-site mismatch at {site:#x}')
        if target in LEAF:
            end, sha = LEAF[target]
            raw = code.read(target, end-target)
            if target in index.by_begin or digest(raw) != sha:
                raise ExportError('pinned leaf identity changed')
            chunks = [{'begin_rva': target, 'end_rva': end, 'raw_hex': raw.hex().upper(),
                       'sha256': sha, 'unwind': None}]
            scope = 'pinned_leaf_CFG'
        else:
            chunks = []
            for f in index.group(target):
                raw = code.read(f.begin, f.end-f.begin)
                chunks.append({'begin_rva': f.begin, 'end_rva': f.end,
                    'raw_hex': raw.hex().upper(), 'sha256': digest(raw), 'unwind': index.info[f.begin]})
            scope = 'complete_unwind_linked_group_NOT_transitive_call_graph'
        functions.append({'entry_rva': target, 'call_site_rva': site, 'call_bytes': sig,
            'scope': scope, 'ranges': chunks,
            'total_code_bytes': sum(x['end_rva']-x['begin_rva'] for x in chunks),
            'transitive_callees_complete': False, 'semantic_role_proved_by_export': False})
    return {'schema': 'nioh3.augmentation-code-closure.v1', 'capture_kind': 'offline_only',
            'section_hashes': {k: digest(v) for k, v in [('text',text),('rdata',rdata),('pdata',pdata)]},
            'functions': functions, 'uses_process': False, 'breakpoints_used': 0,
            'game_acceptance': False}


def objdump_range(raw: bytes, rva: int, program: str = 'objdump') -> tuple[str, list[dict]]:
    """Optional independent decoder; fail on missing/bad instruction coverage."""
    with tempfile.TemporaryDirectory() as td:
        path = Path(td)/'range.bin'
        path.write_bytes(raw)
        run = subprocess.run([program,'-D','-b','binary','-m','i386:x86-64','-Mintel',
                              '--insn-width=16',f'--adjust-vma={rva}',str(path)],
                             capture_output=True,text=True,check=True,timeout=20)
    decoded = []
    for match in re.finditer(r'^\s*([0-9a-f]+):\s*((?:[0-9a-f]{2}\s+)+)\s*([^\n]+)$',run.stdout,re.M):
        a = int(match[1],16); b = bytes.fromhex(match[2]); ins = match[3].strip()
        if '(bad)' in ins or ins.startswith('.byte') or ins.startswith('.word'):
            raise ExportError(f'decode failed at {a:#x}')
        decoded.append({'rva':a,'bytes':b.hex().upper(),'instruction':ins})
    next_address = rva
    for row in decoded:
        if row['rva'] != next_address:
            raise ExportError('decoder left an instruction gap')
        next_address += len(bytes.fromhex(row['bytes']))
    if next_address != rva+len(raw):
        raise ExportError('decoder did not cover complete range')
    return run.stdout.replace(str(path),'range.bin'), decoded


def write_export(result: dict, output: Path, objdump: str | None = None) -> None:
    # A failed decoder may leave a partial directory, never a success manifest.
    output.mkdir(parents=True, exist_ok=False)
    for fn in result['functions']:
        decoded = []
        asm = []
        for part in fn['ranges']:
            name = f"{fn['entry_rva']:08X}_{part['begin_rva']:08X}.bin"
            raw = bytes.fromhex(part['raw_hex'])
            (output/name).write_bytes(raw)
            part['file'] = name
            if objdump:
                text, rows = objdump_range(raw,part['begin_rva'],objdump)
                asm.append(text);decoded.extend(rows)
        if objdump:
            (output/f"{fn['entry_rva']:08X}.asm").write_text('\n'.join(asm),encoding='utf-8')
            boundaries = {r['rva'] for r in decoded}
            edges = []
            for row in decoded:
                words = row['instruction'].split()
                mnemonic = words[0]
                if mnemonic.startswith('j') or mnemonic.startswith('loop') or mnemonic == 'call':
                    try: target = int(words[1],16)
                    except (ValueError, IndexError): target = None
                    kind = 'direct_call' if mnemonic == 'call' else 'branch'
                    if target is None:kind = 'indirect_call' if mnemonic == 'call' else 'indirect_branch'
                    edges.append({'site':row['rva'],'target':target,'kind':kind,
                                  'in_exported_instruction_boundaries':target in boundaries})
            fn['control_flow_edges'] = edges
            fn['decoded_instruction_count'] = len(decoded)
            fn['branch_targets_covered'] = all(e['in_exported_instruction_boundaries'] for e in edges
                                              if e['kind'] not in ('direct_call','indirect_call'))
            # This does not include callees, EH handlers or runtime indirect targets.
    with (output/'FUNCTION_CLOSURES.json').open('x',encoding='utf-8') as f:
        json.dump(result,f,indent=2);f.write('\n')


def main() -> None:
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument('--text',type=Path,required=True)
    p.add_argument('--rdata',type=Path,required=True)
    p.add_argument('--pdata',type=Path,required=True)
    p.add_argument('--output',type=Path,required=True)
    p.add_argument('--objdump',default=None,help='Optional GNU objdump executable for decode/branch checks')
    a = p.parse_args()
    result = build_export(a.text.read_bytes(),a.rdata.read_bytes(),a.pdata.read_bytes())
    write_export(result,a.output,a.objdump)
    print(f"Exported {len(result['functions'])} unwind groups/leaves to {a.output}; no game access")


if __name__ == '__main__':
    main()
