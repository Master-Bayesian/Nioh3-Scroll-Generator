"""Offline, bounded comparison of PC v2.01 insertion and inventory captures.

This tool reads JSON only. It cannot establish natural acquisition, native-call
safety, or successful insertion from a return record. In the observed pickup
caller, the output buffer represents a remainder; inventory readback is separate
evidence. Existing inventory records may include experiments.
"""
from __future__ import annotations

import argparse
from collections import Counter
import hashlib
import json
from pathlib import Path
import struct


RECORD_SIZE = 0xE8
MAX_FILE_BYTES = 16 * 1024 * 1024
MAX_EVENTS = 8192
MAX_RECORDS = 4096
SCROLL_TYPES = {0x1E82: 1, 0x516D: 2, 0xE604: 3, 0xDD82: 4, 0xD523: 5}


class EvidenceError(ValueError):
    """The capture cannot support an unambiguous comparison."""


def integer(value, label, *, maximum=0xFFFFFFFFFFFFFFFF):
    if isinstance(value, str) and value.startswith(('0x', '0X')):
        try:
            value = int(value, 16)
        except ValueError as error:
            raise EvidenceError(f'{label}: invalid hexadecimal integer') from error
    if type(value) is not int or not 0 <= value <= maximum:
        raise EvidenceError(f'{label}: expected an unsigned integer')
    return value


def record(value, label):
    if not isinstance(value, str) or len(value) != RECORD_SIZE * 2:
        raise EvidenceError(f'{label}: expected exactly {RECORD_SIZE} bytes')
    try:
        result = bytes.fromhex(value)
    except ValueError as error:
        raise EvidenceError(f'{label}: invalid hexadecimal record') from error
    if len(result) != RECORD_SIZE:
        raise EvidenceError(f'{label}: whitespace is not accepted')
    return result


def provenance(value=None):
    """Preserve explicit experimental labels; never upgrade unknown records."""
    if value is None:
        return {'kind': 'unknown', 'basis': 'No independent origin evidence supplied.'}
    if not isinstance(value, dict) or value.get('kind') not in ('unknown', 'experimental'):
        raise EvidenceError('Provenance must be unknown or experimental; natural acquisition is not established by these inputs')
    result = dict(value)
    if result['kind'] == 'experimental' and not isinstance(result.get('evidence_ref'), str):
        raise EvidenceError('Experimental provenance requires an explicit evidence_ref')
    if result['kind'] == 'experimental' and not result['evidence_ref'].strip():
        raise EvidenceError('Experimental evidence_ref must not be empty')
    return result


def fields(raw):
    record_type = struct.unpack_from('<H', raw)[0]
    flags = struct.unpack_from('<I', raw, 0x18)[0]
    # PC v2.01 RVA 0x2FA624..0x2FA64E: the constant-one flag has
    # precedence over the uint32-stack flag. Other records use the low word.
    if flags & 0x00800000:
        quantity, encoding = 1, 'constant_one_flag_00800000'
    elif flags & 0x00200000:
        quantity, encoding = struct.unpack_from('<I', raw, 4)[0], 'uint32_at_04'
    else:
        quantity, encoding = struct.unpack_from('<H', raw, 4)[0], 'uint16_at_04'
    serial = struct.unpack_from('<Q', raw, 0x28)[0]
    return {
        'record_type': f'0x{record_type:04X}',
        'quantity': quantity,
        'quantity_encoding': encoding,
        'record_flags_u32': flags,
        'serial_u64': serial,
        'serial_hex': f'0x{serial:016X}',
        'seed': struct.unpack_from('<I', raw, 0x20)[0] if record_type in SCROLL_TYPES else None,
        'is_scroll': record_type in SCROLL_TYPES,
        'sha256': hashlib.sha256(raw).hexdigest(),
    }


def changed_offsets(before, after):
    return [f'0x{index:02X}' for index, (old, new) in enumerate(zip(before, after)) if old != new]


def analyze_capture(capture, owners=None):
    if not isinstance(capture, dict) or capture.get('schema') != 'nioh3-live-insertion-observation/v1':
        raise EvidenceError('Unsupported insertion capture schema')
    capture_origin = provenance(capture.get('provenance'))
    events = capture.get('events')
    if not isinstance(events, list) or len(events) > MAX_EVENTS:
        raise EvidenceError('Capture events must be a bounded list')
    pid = integer(capture.get('pid'), 'capture.pid')
    if owners is not None:
        if not isinstance(owners, dict) or owners.get('schema') != 'nioh3-observed-stack-owners/v1':
            raise EvidenceError('Unsupported stack-owner schema')
        if integer(owners.get('pid'), 'owners.pid') != pid:
            raise EvidenceError('Stack-owner PID does not match the insertion capture')
    entries, returns, pairs = {}, set(), []
    scroll_entries = 0
    for index, event in enumerate(events):
        if not isinstance(event, dict):
            raise EvidenceError(f'events[{index}] must be an object')
        origin = provenance(event.get('provenance', capture_origin))
        kind = event.get('kind')
        if kind == 'entry':
            sequence = integer(event.get('sequence'), 'entry.sequence')
            if sequence == 0 or sequence in entries:
                raise EvidenceError('Entry sequences must be positive and unique')
            source = record(event.get('source_hex'), 'entry.source_hex')
            source_fields = fields(source)
            if integer(event.get('record_type'), 'entry.record_type') != int(source_fields['record_type'], 16):
                raise EvidenceError('Entry record_type disagrees with source readback')
            if type(event.get('is_scroll')) is not bool or event['is_scroll'] != source_fields['is_scroll']:
                raise EvidenceError('Entry is_scroll disagrees with source readback')
            if 'seed' in event and event['seed'] != source_fields['seed']:
                raise EvidenceError('Entry seed disagrees with source readback')
            output_address = integer(event.get('rdx'), 'entry.rdx')
            stack = integer(event.get('rsp'), 'entry.rsp')
            if output_address == 0 or stack == 0:
                raise EvidenceError('Entry output and stack addresses must be nonzero')
            scroll_entries += source_fields['is_scroll']
            entries[sequence] = (event, source, source_fields, origin, output_address, stack)
        elif kind == 'return':
            sequence = integer(event.get('entry_sequence'), 'return.entry_sequence')
            if sequence not in entries:
                raise EvidenceError('Return has no preceding entry')
            if sequence in returns:
                raise EvidenceError('Duplicate return for one entry')
            entry, source, source_fields, entry_origin, output_address, stack = entries[sequence]
            if integer(event.get('output_address'), 'return.output_address') != output_address:
                raise EvidenceError('Return output_address disagrees with entry RDX')
            if integer(event.get('rax'), 'return.rax') != output_address:
                raise EvidenceError('Return RAX disagrees with the output address')
            if integer(event.get('rsp'), 'return.rsp') != stack + 8:
                raise EvidenceError('Return RSP does not match the captured entry stack')
            output = record(event.get('output_record_hex'), 'return.output_record_hex')
            source_after = record(event.get('source_after_hex'), 'return.source_after_hex')
            output_fields = fields(output)
            owners_at_capture = []
            for owner in (owners or {}).get('matches', []):
                pointers = [integer(value, 'owner.matched_pointer') for value in owner.get('matched_pointers', [])]
                if stack in pointers:
                    low = integer(owner.get('stack_limit'), 'owner.stack_limit')
                    high = integer(owner.get('stack_base'), 'owner.stack_base')
                    if not low <= stack < high:
                        raise EvidenceError('Matched stack pointer falls outside owner bounds')
                    owners_at_capture.append(integer(owner.get('thread_id'), 'owner.thread_id'))
            if len(owners_at_capture) > 1:
                raise EvidenceError('Ambiguous stack ownership')
            pairs.append({
                'entry_sequence': sequence, 'caller_rva': entry.get('caller_rva'),
                'provenance': entry_origin, 'return_provenance': origin,
                'source': source_fields, 'output': output_fields,
                'source_changed_offsets': changed_offsets(source, source_after),
                'source_to_output_changed_offsets': changed_offsets(source, output),
                'output_is_empty': int(output_fields['record_type'], 16) == 0,
                'output_type_matches_source_or_empty': output_fields['record_type'] in ('0x0000', source_fields['record_type']),
                'source_minus_output_quantity': source_fields['quantity'] - output_fields['quantity'],
                'observed_stack_owner_thread_id': owners_at_capture[0] if owners_at_capture else None,
                'insertion_success': 'not_established_by_return_buffer',
            })
            returns.add(sequence)
        else:
            raise EvidenceError(f'Unsupported event kind: {kind!r}')
    entry_hits = integer(capture.get('entry_hits'), 'capture.entry_hits')
    scroll_hits = integer(capture.get('scroll_hits'), 'capture.scroll_hits')
    if entry_hits < len(entries) or scroll_hits < scroll_entries or scroll_hits > entry_hits:
        raise EvidenceError('Capture counters disagree with recorded events')
    return {
        'pid': pid, 'provenance': capture_origin,
        'total_entry_hits': entry_hits, 'recorded_entries': len(entries),
        'unrecorded_entry_hits': entry_hits - len(entries), 'recorded_scroll_entries': scroll_entries,
        'paired_count': len(pairs), 'unpaired_entry_sequences': sorted(set(entries) - returns),
        'source_unchanged_pair_count': sum(not pair['source_changed_offsets'] for pair in pairs),
        'empty_output_pair_count': sum(pair['output_is_empty'] for pair in pairs),
        'pairs': pairs,
    }


def inventory_entries(snapshot):
    if not isinstance(snapshot, dict):
        raise EvidenceError('Inventory snapshot must be an object')
    default_origin = provenance(snapshot.get('provenance'))
    entries = snapshot.get('entries')
    if not isinstance(entries, list) or len(entries) > MAX_RECORDS:
        raise EvidenceError('Inventory entries must be a bounded list')
    result, slots = {}, set()
    for entry in entries:
        if not isinstance(entry, dict):
            raise EvidenceError('Inventory entry must be an object')
        slot = integer(entry.get('slot_index'), 'inventory.slot_index')
        raw = record(entry.get('record_hex'), 'inventory.record_hex')
        info = fields(raw)
        if not info['is_scroll']:
            raise EvidenceError('Scroll snapshot contains a non-scroll record')
        serial = info['serial_u64']
        if serial in (0, 0xFFFFFFFFFFFFFFFF) or serial in result or slot in slots:
            raise EvidenceError('Inventory serials and slots must be valid and unique')
        for key, expected in (('serial', serial), ('serial_u64', serial), ('seed', info['seed']),
                              ('playthrough', SCROLL_TYPES[int(info['record_type'], 16)]),
                              ('rarity', raw[0x30])):
            if key in entry and entry[key] != expected:
                raise EvidenceError(f'Inventory {key} disagrees with record readback')
        slots.add(slot)
        result[serial] = (raw, {**info, 'slot_index': slot,
                               'provenance': provenance(entry.get('provenance', default_origin))})
    return result


def compare_inventories(before, after):
    old, new = inventory_entries(before), inventory_entries(after)
    common = sorted(old.keys() & new.keys())
    matched = [{
        'serial_u64': serial, 'before': old[serial][1], 'after': new[serial][1],
        'record_equal': old[serial][0] == new[serial][0],
        'changed_offsets': changed_offsets(old[serial][0], new[serial][0]),
    } for serial in common]
    return {
        'identity_basis': 'Full uint64 serial at record +0x28, unique within each supplied snapshot; full bytes checked independently. Never low-word or seed matching.',
        'scope': 'Snapshot differences cannot establish acquisition, persistence, causality, or natural origin.',
        'before_count': len(old), 'after_count': len(new), 'matched_count': len(common),
        'unchanged_record_count': sum(row['record_equal'] for row in matched),
        'changed_record_count': sum(not row['record_equal'] for row in matched),
        'moved_slot_count': sum(old[serial][1]['slot_index'] != new[serial][1]['slot_index'] for serial in common),
        'added': [new[serial][1] for serial in sorted(new.keys() - old.keys())],
        'removed': [old[serial][1] for serial in sorted(old.keys() - new.keys())],
        'after_provenance_counts': dict(Counter(info['provenance']['kind'] for _, info in new.values())),
        'matched': matched,
    }


def load_json(path):
    with path.open('rb') as stream:
        raw = stream.read(MAX_FILE_BYTES + 1)
    if len(raw) > MAX_FILE_BYTES:
        raise EvidenceError(f'{path}: input exceeds the {MAX_FILE_BYTES}-byte limit')
    def unique_object(items):
        result = {}
        for key, value in items:
            if key in result:
                raise EvidenceError(f'{path}: duplicate JSON key {key!r}')
            result[key] = value
        return result
    try:
        data = json.loads(raw.decode('utf-8-sig'), object_pairs_hook=unique_object,
                          parse_constant=lambda token: (_ for _ in ()).throw(EvidenceError(f'Invalid JSON constant: {token}')))
    except (UnicodeError, json.JSONDecodeError, RecursionError) as error:
        raise EvidenceError(f'{path}: invalid JSON') from error
    return data, {'path': str(path.resolve()), 'sha256': hashlib.sha256(raw).hexdigest(), 'byte_size': len(raw)}


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--capture', required=True, type=Path)
    parser.add_argument('--before', required=True, type=Path)
    parser.add_argument('--after', required=True, type=Path)
    parser.add_argument('--owners', type=Path)
    parser.add_argument('--output', required=True, type=Path)
    args = parser.parse_args()
    inputs = {name: load_json(getattr(args, name)) for name in ('capture', 'before', 'after')}
    if args.owners:
        inputs['owners'] = load_json(args.owners)
    output = {
        'schema': 'nioh3-live-insertion-evidence-analysis/v1',
        'offline_only': True, 'natural_generation_evidence': False,
        'limitations': [
            'Initial inventory includes possible experimental records and is not a clean natural sample.',
            'The pickup caller consumes the returned record as a remainder; empty output alone does not prove inventory insertion.',
            'Source preservation and matched stack bounds do not establish arbitrary-thread call safety.',
            'This tool does not establish a causal link between capture events and snapshot differences.',
        ],
        'inputs': {key: info for key, (_, info) in inputs.items()},
        'capture': analyze_capture(inputs['capture'][0], inputs.get('owners', (None,))[0]),
        'inventory': compare_inventories(inputs['before'][0], inputs['after'][0]),
    }
    if args.output.resolve() in {Path(info['path']) for _, info in inputs.values()}:
        raise EvidenceError('Output must not overwrite any input evidence')
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(json.dumps(output, indent=2) + '\n', encoding='utf-8')
    print(json.dumps({'output': str(args.output), 'paired_count': output['capture']['paired_count'],
                      'inventory_before': output['inventory']['before_count'],
                      'inventory_after': output['inventory']['after_count'],
                      'natural_generation_evidence': False}))


if __name__ == '__main__':
    main()
