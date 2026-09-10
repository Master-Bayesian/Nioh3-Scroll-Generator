"""Read-only verification of bounded insertion scheduling disassembly evidence."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import sys

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
from research.dump_effect_catalog_current_locale import ProcessReader
from nioh3_scroll_editor.runtime_application import running_game_identity


def verify(directory):
    pid, profile, _ = running_game_identity()
    if profile.display_version != 'PC v2.01':
        raise ValueError('Scheduling evidence requires PC v2.01')
    reports = [directory / name for name in
               ('scheduling-callees.json', 'scheduling-callers.json')]
    functions = {}
    sources = []
    with ProcessReader() as reader:
        if reader.pid != pid:
            raise RuntimeError('Process changed during identity verification')
        for rva, signature in profile.native_signatures:
            if reader.read(reader.module_base + rva, len(signature)) != signature:
                raise RuntimeError('Runtime profile signature mismatch')
        for path in reports:
            raw = path.read_bytes()
            report = json.loads(raw)
            if report['schema'] != 'nioh3-static-function-disassembly/v1':
                raise ValueError('Unexpected disassembly schema')
            sources.append({'file': path.name, 'sha256': hashlib.sha256(raw).hexdigest()})
            for function in report['functions']:
                start, end = int(function['begin_rva'], 0), int(function['end_rva'], 0)
                cursor, expected, instructions = start, bytearray(), {}
                for instruction in function['instructions']:
                    rva = int(instruction['rva'], 0)
                    code = bytes.fromhex(instruction['bytes_hex'])
                    if rva != cursor or not code:
                        raise ValueError('Noncontiguous instruction evidence')
                    instructions[rva] = instruction
                    expected.extend(code)
                    cursor += len(code)
                if cursor != end:
                    raise ValueError('Disassembly does not cover the entire pdata range')
                actual = reader.read(reader.module_base + start, len(expected))
                if actual != expected:
                    raise RuntimeError(f'Live code differs at range {start:#x}')
                functions[start] = {'begin_rva': hex(start), 'end_rva': hex(end),
                                    'byte_count': len(actual),
                                    'sha256': hashlib.sha256(actual).hexdigest(),
                                    'instructions': instructions}
        xref_path = directory / 'scheduling-xrefs-raw.json'
        raw = xref_path.read_bytes()
        sources.append({'file': xref_path.name, 'sha256': hashlib.sha256(raw).hexdigest()})
        calls = []
        for xref in json.loads(raw)['xrefs']:
            function = functions[int(xref['caller_begin_rva'], 0)]
            instruction = function['instructions'].get(int(xref['call_rva'], 0))
            if not instruction or instruction['mnemonic'] != 'call':
                raise ValueError('Byte-pattern xref is not a decoded call boundary')
            if int(instruction['operands'], 0) != int(xref['target_rva'], 0):
                raise ValueError('Decoded call target mismatch')
            calls.append(xref)
        # Recheck the same bounded ranges before claiming consistent live bytes.
        for function in functions.values():
            actual = reader.read(reader.module_base + int(function['begin_rva'], 0), function['byte_count'])
            if hashlib.sha256(actual).hexdigest() != function['sha256']:
                raise RuntimeError('Code changed during verification')
            del function['instructions']
    return {'schema': 'nioh3-live-scheduling-evidence/v1',
            'captured_at_utc': datetime.now(timezone.utc).isoformat(),
            'pid': pid, 'game_version': profile.display_version, 'read_only': True,
            'sources': sources, 'ranges': list(functions.values()), 'validated_direct_calls': calls,
            'limitations': ['Pdata ranges can be fragments, not standalone callable functions.',
                            'Direct calls do not cover indirect dispatch or tail jumps.',
                            'Code identity does not establish thread ownership or safe invocation.']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--evidence-directory', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    result = verify(args.evidence_directory)
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(result, stream, indent=2)
        stream.write('\n')
    print(json.dumps({'pid': result['pid'], 'verified_ranges': len(result['ranges']),
                      'validated_direct_calls': len(result['validated_direct_calls'])}))
