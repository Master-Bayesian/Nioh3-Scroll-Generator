"""Check recorded one-shot scheduling invariants without accessing the game."""
import argparse
import hashlib
import json
from pathlib import Path


REGISTERS = ('RAX', 'RBX', 'RCX', 'RDX', 'RSI', 'RDI', 'RBP',
             'R8', 'R9', 'R10', 'R11', 'R12', 'R13', 'R14', 'R15')


def verify_dispatch(execution):
    if execution.get('phase') != 'completed' or execution.get('redirect_count') != 1:
        raise ValueError('Exactly one acknowledged redirect is required')
    if execution.get('released') is not True or execution.get('breakpoints') != []:
        raise ValueError('Allocation or breakpoint cleanup is not confirmed')
    before, after = execution['before'], execution['after']
    if any(before[key] != after[key] for key in REGISTERS):
        raise ValueError('A general register changed')
    if before['RSP'] % 16 != 8 or after['RSP'] != before['RSP'] - 0x48:
        raise ValueError('Unexpected stack alignment or prologue delta')
    if after['RIP'] != before['RIP'] + 7:
        raise ValueError('Continuation is not the end of the replayed prologue')
    # push rbx; push rdi; sub rsp,0x38. Only SUB changes arithmetic flags.
    left = before['RSP'] - 16
    result = (left - 0x38) & ((1 << 64) - 1)
    expected = ((left < 0x38) | (((result & 255).bit_count() % 2 == 0) << 2)
                | ((((left ^ 0x38 ^ result) & 16) != 0) << 4)
                | ((result == 0) << 6) | (((result >> 63) & 1) << 7)
                | ((((left ^ 0x38) & (left ^ result) & (1 << 63)) != 0) << 11))
    if after['EFLAGS'] & 0x8D5 != expected:
        raise ValueError('Arithmetic flags do not match the original prologue')
    # Debugger RF/TF are not product flags and may differ at hardware stops.
    if (before['EFLAGS'] ^ after['EFLAGS']) & ~(0x8D5 | 0x10100):
        raise ValueError('Non-arithmetic flags changed')


def verify(execution, before_inventory, after_inventory):
    if execution.get('mode') not in ('no_call', 'read_only_slot_lookup', 'assembly_preview'):
        raise ValueError('Use the mutation verifier for insertion evidence')
    verify_dispatch(execution)
    for field in ('pid', 'capacity', 'entries', 'serial_counter', 'acquisition_order_counter',
                  'container_sha256', 'duplicate_scroll_serials'):
        if before_inventory[field] != after_inventory[field]:
            raise ValueError(f'Inventory comparison differs: {field}')
    if execution.get('mode') == 'read_only_slot_lookup' and execution['actual'] != execution['expected']:
        raise ValueError('Native query result differs')
    return {'schema': 'nioh3-dispatch-probe-verification/v1',
            'general_registers_preserved': len(REGISTERS), 'prologue_stack_and_flags_match': True,
            'scroll_records_unchanged': len(before_inventory['entries']),
            'serial_and_acquisition_counters_unchanged': True,
            'one_shot_and_cleanup_confirmed': True,
            'native_read_only_query': execution.get('mode') == 'read_only_slot_lookup',
            'limits': ['Does not prove safe item mutation, serial allocation exclusion, or all game states.',
                       'XMM save/restore instructions are present in the call shim; XMM values were not captured.',
                       'Inventory checks are bounded before/after snapshots, not continuous observation.']}


if __name__ == '__main__':
    parser = argparse.ArgumentParser()
    parser.add_argument('--execution', type=Path, required=True)
    parser.add_argument('--before', type=Path, required=True)
    parser.add_argument('--after', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    paths = (args.execution, args.before, args.after)
    raw = [path.read_bytes() for path in paths]
    report = verify(*(json.loads(value) for value in raw))
    report['sources'] = [{'file': path.name, 'sha256': hashlib.sha256(value).hexdigest()}
                         for path, value in zip(paths, raw)]
    with args.output.open('x', encoding='utf-8') as stream:
        json.dump(report, stream, indent=2)
        stream.write('\n')
    print(json.dumps(report))
