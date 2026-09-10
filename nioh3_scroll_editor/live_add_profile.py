"""Version-owned live-add layout; discovery does not grant mutation support.

Keep game RVAs and structure offsets here. ABI/shim offsets belong to the
executor, not to this profile. Changing an RVA alone cannot accept a new build.
"""
from dataclasses import asdict, dataclass


@dataclass(frozen=True, slots=True)
class LiveAddProfile:
    profile_id: str
    file_version: tuple[int, int, int, int]
    dispatch_rva: int
    dispatch_return_rva: int
    dispatch_signature_hex: str
    builder_rva: int
    builder_size: int
    insertion_rva: int
    insertion_size: int
    slot_lookup_rva: int
    manager_pointer_rva: int
    scheduler_pointer_rva: int
    container_offset: int
    capacity_offset: int
    serial_counter_offset: int
    serial_index_offset: int
    scheduler_pending_offset: int
    scheduler_ready_offset: int
    queue_begin_offset: int
    queue_end_offset: int
    record_size: int = 0xE8
    descriptor_size: int = 0xCC
    capacity: int = 400

    def lua_layout(self):
        return {key: value for key, value in asdict(self).items()
                if key not in ('file_version', 'profile_id')}


PC_V201 = LiveAddProfile(
    profile_id='pc-v2.01-live-add-r1', file_version=(2, 0, 1, 0),
    dispatch_rva=0x12E6840, dispatch_return_rva=0x20BB2C,
    dispatch_signature_hex='4053574883EC38',
    builder_rva=0x227C4CC, builder_size=0x27B,
    insertion_rva=0x54D294, insertion_size=0xE17,
    slot_lookup_rva=0x552FBC, manager_pointer_rva=0x474D4E0,
    scheduler_pointer_rva=0x47412F8, container_offset=0x224A60,
    capacity_offset=0x16A80, serial_counter_offset=8,
    serial_index_offset=0x23B5E8, scheduler_pending_offset=0x1408,
    scheduler_ready_offset=0x1629, queue_begin_offset=0x60, queue_end_offset=0x68)


def live_add_profile(version):
    if tuple(version) != PC_V201.file_version:
        raise ValueError('Live insertion has not been accepted for this game version')
    return PC_V201
