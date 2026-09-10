"""Version-gated read-only scroll inventory and native serial-index inspection."""
from datetime import datetime, timezone
import hashlib
import struct
from .process_memory_readonly import ProcessReader
from .live_add_profile import PC_V201 as LAYOUT

def running_game_identity():
    from .runtime_application import running_game_identity as identity
    return identity()

def capture_inventory():
    pid, profile, _ = running_game_identity()
    if profile.display_version != 'PC v2.01':
        raise ValueError('This inventory layout is validated only for PC v2.01')
    with ProcessReader() as reader:
        if reader.pid != pid:
            raise RuntimeError('Game process changed during identity verification')
        signature = bytes.fromhex('40 55 53 56 57 41 54 41 55 41 56 41 57 48 8D AC')
        if reader.read(reader.module_base + LAYOUT.insertion_rva, len(signature)) != signature:
            raise RuntimeError('Inventory insertion signature mismatch')
        manager_address = reader.module_base + LAYOUT.manager_pointer_rva
        manager = reader.u64(manager_address)
        if not manager:
            raise RuntimeError('Item manager is not loaded')
        data = reader.u64(manager)
        if not data:
            raise RuntimeError('Inventory data is not loaded')
        container = data + LAYOUT.container_offset
        if reader.u64(container + LAYOUT.capacity_offset) != 400:
            raise RuntimeError('Unexpected scroll container capacity')
        counters = reader.read(data, 16)
        raw = reader.read(container, 400 * 0xE8)
        if raw != reader.read(container, len(raw)) or counters != reader.read(data, 16):
            raise RuntimeError('Inventory or counters changed during capture; retry at rest')
        if reader.u64(manager_address) != manager or reader.u64(manager) != data:
            raise RuntimeError('Inventory owner changed during capture')
        entries = []
        for slot in range(400):
            record = raw[slot * 0xE8:(slot + 1) * 0xE8]
            if struct.unpack_from('<H', record)[0]:
                entries.append({'slot_index': slot, 'record_hex': record.hex(),
                                'serial': str(struct.unpack_from('<Q', record, 0x28)[0]),
                                'seed': struct.unpack_from('<I', record, 0x20)[0]})
        serials = [entry['serial'] for entry in entries]
        return {'schema': 'nioh3-live-scroll-readonly/v1', 'captured_at_utc': datetime.now(timezone.utc).isoformat(),
                'pid': pid, 'game_version': profile.display_version, 'read_only': True,
                'capacity': 400, 'entries': entries, 'duplicate_scroll_serials': sorted({s for s in serials if serials.count(s) > 1}),
                'serial_counter': str(struct.unpack_from('<Q', counters, 8)[0]),
                'acquisition_order_counter': struct.unpack_from('<I', counters)[0],
                'container_sha256': hashlib.sha256(raw).hexdigest(),
                'consistency': 'Matching whole-container, counters and owner reads; not an atomic engine snapshot',
                'scope': 'Inventory inspection only; no native invocation, serial reservation, mutation or proof of thread safety'}


def fnv(serial):
    value = 0xCBF29CE484222325
    for byte in struct.pack('<Q', serial):
        value = ((value ^ byte) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return value


def inspect(reader, address):
    header = reader.read(address, 0x40)
    head, size, buckets = struct.unpack_from('<QQQ', header, 8)
    mask, bucket_count = struct.unpack_from('<QQ', header, 0x30)
    if not head or not buckets or not 0 < bucket_count <= 1 << 20 or mask != bucket_count - 1 or bucket_count & mask or size > 10000:
        raise ValueError('Invalid bounded serial-index header')
    nodes, seen = {}, set()
    node = reader.u64(head)
    snapshots = []
    while node != head:
        if not node or node in seen or len(seen) >= size:
            raise ValueError('Invalid serial-index list topology')
        seen.add(node)
        raw = reader.read(node, 0x20)
        next_node, previous, serial, slot = struct.unpack_from('<QQQI', raw)
        if serial in nodes:
            raise ValueError('Duplicate full serial index key')
        nodes[serial] = {'slot': slot, 'node': node}
        snapshots.append((node, raw))
        node = next_node
    if len(nodes) != size:
        raise ValueError('Index list size mismatch')
    for serial, entry in nodes.items():
        start = buckets + (fnv(serial) & mask) * 16
        raw_bucket = reader.read(start, 16)
        first, current = struct.unpack('<QQ', raw_bucket)
        traversed = set()
        while current != head:
            if current in traversed or current not in seen:
                raise ValueError('Invalid hash bucket topology')
            traversed.add(current)
            raw = reader.read(current, 0x20)
            if struct.unpack_from('<Q', raw, 0x10)[0] == serial:
                if current != entry['node']:
                    raise ValueError('Bucket/list lookup disagreement')
                break
            if current == first:
                raise ValueError('Serial missing from its FNV bucket')
            current = struct.unpack_from('<Q', raw, 8)[0]
        else:
            raise ValueError('Empty bucket for an indexed serial')
        snapshots.append((start, raw_bucket))
    if header != reader.read(address, 0x40) or any(raw != reader.read(a, len(raw)) for a, raw in snapshots):
        raise ValueError('Serial index changed during capture')
    return {'node_count': size, 'bucket_count': bucket_count,
            'entries': [{'serial': str(serial), 'slot': item['slot']} for serial, item in sorted(nodes.items())]}


def capture_index():
    pid, profile, _ = running_game_identity()
    if profile.display_version != 'PC v2.01':
        raise ValueError('PC v2.01 is required')
    with ProcessReader() as reader:
        if reader.pid != pid:
            raise ValueError('Process changed')
        manager = reader.u64(reader.module_base + LAYOUT.manager_pointer_rva)
        data = reader.u64(manager)
        if reader.u64(data + LAYOUT.container_offset + LAYOUT.capacity_offset) != 400:
            raise ValueError('Unexpected inventory owner')
        result = inspect(reader, data + LAYOUT.serial_index_offset)
        if reader.u64(reader.module_base + LAYOUT.manager_pointer_rva) != manager or reader.u64(manager) != data:
            raise ValueError('Inventory owner changed')
    return {'schema': 'nioh3-native-serial-index/v1', 'pid': pid, 'read_only': True,
            'captured_at_utc': datetime.now(timezone.utc).isoformat(), **result,
            'scope': 'Native FNV bucket/list agreement; historical unused keys may remain. Not an atomic snapshot.'}
