"""Independent full-container, index and save verification for live addition."""
import hashlib
import struct
from .dispatch_evidence import verify_dispatch

def record(value):
    raw = bytes.fromhex(value)
    if len(raw) != 0xE8:
        raise ValueError('Invalid scroll record length')
    return raw


def inventory_entries(value):
    entries, slots = {}, set()
    if value['capacity'] != 400 or value['duplicate_scroll_serials']:
        raise ValueError('Invalid inventory capacity or duplicate serials')
    for entry in value['entries']:
        raw = record(entry['record_hex'])
        serial, slot = entry['serial'], entry['slot_index']
        if (serial in entries or slot in slots or not 0 <= slot < 400
                or str(struct.unpack_from('<Q', raw, 0x28)[0]) != serial
                or struct.unpack_from('<I', raw, 0x20)[0] != entry['seed']
                or not struct.unpack_from('<H', raw)[0]):
            raise ValueError('Invalid or duplicate occupied record')
        entries[serial] = entry
        slots.add(slot)
    return entries


def index_entries(value):
    entries = {item['serial']: item['slot'] for item in value['entries']}
    if len(entries) != len(value['entries']) or len(entries) != value['node_count']:
        raise ValueError('Duplicate keys or incorrect index node count')
    return entries


def defined(raw):
    return raw[:0x24] + raw[0x28:0xE4]


def verify(plan, execution, before, after, index_before, index_after):
    verify_dispatch(execution)
    if (execution.get('mode') != 'single_native_insertion'
            or execution.get('operation_id') != plan['operation_id']
            or execution.get('status') != 3 or execution.get('slot') != plan['slot']):
        raise ValueError('Insertion acknowledgement does not match the plan')
    if any(item['pid'] != plan['pid'] for item in (execution, before, after, index_before, index_after)):
        raise ValueError('Process identity differs')
    old, new = inventory_entries(before), inventory_entries(after)
    serial = str(plan['serial'])
    if serial in old or set(new) != set(old) | {serial}:
        raise ValueError('Expected exactly one newly allocated serial')
    if any(old[key] != new[key] for key in old):
        raise ValueError('An existing record changed')
    if new[serial]['slot_index'] != plan['slot']:
        raise ValueError('The added record occupies another slot')
    baseline = bytes.fromhex(plan['container_hex'])
    if len(baseline) != 400 * 0xE8 or hashlib.sha256(baseline).hexdigest() != before['container_sha256']:
        raise ValueError('Planned container does not match the before capture')
    occupied = {slot: baseline[slot*0xE8:(slot+1)*0xE8] for slot in range(400)
                if baseline[slot*0xE8:slot*0xE8+2] != b'\0\0'}
    if occupied != {item['slot_index']: record(item['record_hex']) for item in old.values()}:
        raise ValueError('Before records do not match the complete planned container')
    start = plan['slot'] * 0xE8
    if baseline[start:start + 2] != b'\0\0':
        raise ValueError('Planned slot was not empty')
    destination = record(execution['destination_hex'])
    source = record(execution['source_hex'])
    remainder = record(execution['remainder_hex'])
    if destination != record(new[serial]['record_hex']) or remainder[:2] != b'\0\0':
        raise ValueError('Destination or remainder disagrees with the receipt')
    if struct.unpack_from('<Q', source, 0x28)[0] != plan['serial']:
        raise ValueError('Builder source serial differs')
    if struct.unpack_from('<I', destination, 0x18)[0] != (struct.unpack_from('<I', source, 0x18)[0] | 0x04000080):
        raise ValueError('Native insertion flags differ from the accepted scroll path')
    # Native insertion changes only flags/acquisition order and uninitialized padding.
    if source[:0x18] != destination[:0x18] or source[0x20:0x24] != destination[0x20:0x24] or source[0x28:0xE4] != destination[0x28:0xE4]:
        raise ValueError('Native insertion changed generated content')
    predicted = baseline[:start] + destination + baseline[start + 0xE8:]
    if hashlib.sha256(predicted).hexdigest() != after['container_sha256']:
        raise ValueError('Another container byte changed')
    if (int(before['serial_counter']) != plan['serial']
            or int(after['serial_counter']) != plan['serial'] + 1
            or after['acquisition_order_counter'] != before['acquisition_order_counter'] + 1
            or struct.unpack_from('<I', destination, 0x1C)[0] != before['acquisition_order_counter']):
        raise ValueError('Native counters or acquisition order differ')
    old_index, new_index = index_entries(index_before), index_entries(index_after)
    if serial in old_index or new_index != dict(old_index, **{serial: plan['slot']}):
        raise ValueError('Native index change is not exactly the new full serial')
    if any(new_index.get(key) != item['slot_index'] for key, item in new.items()):
        raise ValueError('Native index does not resolve occupied records')
    return {'schema': 'nioh3-live-add-verification/v1', 'operation_id': plan['operation_id'],
            'serial': serial, 'seed': new[serial]['seed'], 'slot': plan['slot'],
            'previous_records_preserved': len(old), 'count_after': len(new),
            'full_container_and_native_index_verified': True, 'dispatch_and_cleanup_verified': True,
            'persistence_verified': False,
            'limits': ['One observed mission-thread insertion; not all-state concurrency acceptance.',
                       'Persistence requires independent normal save and reload evidence.']}


def verify_persistence(after, saved_records, *, allow_new_marker_clear=False):
    expected = inventory_entries(after)
    actual = {}
    for raw in saved_records:
        if len(raw) != 0xE8:
            raise ValueError('Partial saved record')
        if raw[:2] == b'\0\0':
            continue
        serial = str(struct.unpack_from('<Q', raw, 0x28)[0])
        if serial in actual:
            raise ValueError('Duplicate saved serial')
        actual[serial] = raw
    if set(actual) != set(expected):
        raise ValueError('Saved inventory serial set differs')
    cleared = []
    for key, entry in expected.items():
        previous = bytearray(record(entry['record_hex']))
        current = actual[key]
        if allow_new_marker_clear and previous[0x18] & 2 and current[0x18] == previous[0x18] & ~2:
            previous[0x18] &= ~2
            cleared.append(key)
        if defined(current) != defined(previous):
            raise ValueError('Saved defined record fields differ')
    return {'records_verified': len(actual), 'serials_and_defined_fields_match': True,
            'new_marker_cleared_serials': cleared}
