"""Serialize a canonical installation record into the native assembly input.

This copies inputs; it never reimplements RNG or finalization. An isolated
native preview must match the installation record before allocating a serial.
"""
import struct


def new_assembly_record(record: bytes) -> bytes:
    """Use PC v2.01 builder metadata, never a template's inventory state.

    Acquisition order and owned/seen/equipped flags are not descriptor inputs.
    Native insertion assigns their destination values. Generation bytes remain
    unchanged and are still checked by the isolated builder preview.
    """
    assembly_descriptor(record)
    result = bytearray(record)
    struct.pack_into('<II', result, 0x18, 0x02800002, 0)
    return bytes(result)


def assembly_descriptor(record: bytes, *, allocate_serial=False) -> bytes:
    if len(record) != 0xE8:
        raise ValueError('Expected one canonical scroll installation record')
    record_type = struct.unpack_from('<H', record)[0]
    seed = struct.unpack_from('<I', record, 0x20)[0]
    flags = struct.unpack_from('<I', record, 0x18)[0]
    if record_type not in (0x1E82, 0x516D, 0xE604) or seed == 0 or not flags & 0x800000:
        raise ValueError('Live assembly requires a seeded nonstackable NG1-NG3 scroll')
    if record[0x30] not in (3, 4, 5):
        raise ValueError('Unsupported assembly rarity')
    descriptor = bytearray(0xCC)
    struct.pack_into('<H', descriptor, 0, record_type)
    struct.pack_into('<I', descriptor, 4, struct.unpack_from('<H', record, 6)[0])
    struct.pack_into('<I', descriptor, 8, struct.unpack_from('<H', record, 0x10)[0])
    descriptor[0x0C] = record[0x30]
    descriptor[0x10:0x14] = record[0x20:0x24]
    descriptor[0x14:0x18] = record[0xDC:0xE0]
    identity = (struct.unpack_from('<I', record, 0x14)[0]
                | struct.unpack_from('<H', record, 4)[0] << 32
                | struct.unpack_from('<H', record, 2)[0] << 48)
    struct.pack_into('<Q', descriptor, 0x18, identity)
    descriptor[0x21] = 0 if allocate_serial else 1
    descriptor[0x22] = record[0x0F]
    descriptor[0x24:0xCC] = record[0x34:0xDC]
    return bytes(descriptor)


def verify_assembly_preview(expected: bytes, actual: bytes):
    if len(expected) != 0xE8 or len(actual) != 0xE8:
        raise ValueError('Partial native assembly preview')
    if actual[0x28:0x30] != b'\xff' * 8:
        raise ValueError('Preview allocated or reused an instance serial')
    # Existing native insertion adds acquisition order and seen/new flags. The
    # expected input here must be a generation installation record, not inventory.
    if expected[:0x24] != actual[:0x24] or expected[0x30:0xE4] != actual[0x30:0xE4]:
        raise ValueError('Native assembly differs from the expected installation record')
    return True
