"""Version-owned live-add layout and inventory pointer ABI.

Discovery does not grant mutation support.  Keep game RVAs and structure
offsets here; ABI/shim offsets belong to the executor.  The inventory global
is an explicit ABI choice because one executable may expose ``global ->
manager -> data`` while another may expose ``global -> data`` directly.
Changing an RVA alone cannot accept a new build.
"""
from dataclasses import asdict, dataclass
from enum import Enum
from typing import Callable


class InventoryGlobalMode(str, Enum):
    """How the version-owned inventory global reaches the data object."""

    MANAGER_OBJECT = "manager_object"
    DIRECT_DATA = "direct_data"

    @classmethod
    def parse(cls, value: object) -> "InventoryGlobalMode":
        if isinstance(value, cls):
            return value
        if isinstance(value, str):
            try:
                return cls(value)
            except ValueError as error:
                raise ValueError(f"Unsupported inventory global mode: {value!r}") from error
        raise ValueError("Inventory global mode is required")


@dataclass(frozen=True, slots=True)
class InventoryPointers:
    """Resolved addresses used by the live-add reader/executor.

    ``global_address`` is the module global slot.  ``manager_address`` is the
    manager object only for ``manager_object`` mode; direct-data mode has no
    manager object and therefore stores ``None`` deliberately.
    """

    global_address: int
    manager_address: int | None
    data_address: int


def resolve_inventory_pointers(
    read_u64: Callable[[int], int],
    module_base: int,
    profile: "LiveAddProfile",
) -> InventoryPointers:
    """Resolve the profile's inventory global without guessing its shape.

    The helper is intentionally small and injectable so both addressing modes
    can be tested with synthetic memory.  Missing/invalid modes and unresolved
    profile RVAs fail before any dereference that could be mistaken for a valid
    owner.  A disabled candidate therefore cannot become dispatchable merely
    by supplying a process or a plausible address.
    """

    mode = InventoryGlobalMode.parse(profile.inventory_global_mode)
    profile.require_dispatchable()
    global_rva = profile.manager_pointer_rva
    if not isinstance(global_rva, int) or global_rva <= 0:
        raise ValueError("Inventory global RVA is unresolved")
    global_address = module_base + global_rva
    global_value = read_u64(global_address)
    if not global_value:
        if mode is InventoryGlobalMode.MANAGER_OBJECT:
            # Preserve the shipped PC v2.01 diagnostic exactly.
            raise RuntimeError("Item manager is not loaded")
        raise RuntimeError("Inventory data is not loaded")
    if mode is InventoryGlobalMode.DIRECT_DATA:
        return InventoryPointers(global_address, None, global_value)

    manager_address = global_value
    data_address = read_u64(manager_address)
    if not data_address:
        raise RuntimeError("Inventory data is not loaded")
    return InventoryPointers(global_address, manager_address, data_address)


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
    manager_pointer_rva: int | None
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
    # Required for any selection; ``None`` deliberately fails closed.  The
    # name remains manager_pointer_rva for compatibility with the shipped
    # v2.01 profile, while the ABI mode determines what it points to.
    inventory_global_mode: str | InventoryGlobalMode | None = None
    accepted: bool = True
    candidate_only: bool = False

    def lua_layout(self):
        layout = {key: value for key, value in asdict(self).items()
                  if key not in ('file_version', 'profile_id', 'accepted', 'candidate_only')}
        layout = {key: value for key, value in layout.items() if value is not None}
        mode = layout.get('inventory_global_mode')
        if isinstance(mode, InventoryGlobalMode):
            layout['inventory_global_mode'] = mode.value
        return layout

    @property
    def inventory_global_rva(self) -> int | None:
        """Semantic alias for the version-owned global slot RVA."""

        return self.manager_pointer_rva

    def is_dispatchable(self) -> bool:
        """Whether this profile is eligible for product selection/dispatch."""

        if not self.accepted or self.candidate_only:
            return False
        try:
            InventoryGlobalMode.parse(self.inventory_global_mode)
        except ValueError:
            return False
        required = (
            self.dispatch_rva,
            self.dispatch_return_rva,
            self.builder_rva,
            self.builder_size,
            self.insertion_rva,
            self.insertion_size,
            self.slot_lookup_rva,
            self.manager_pointer_rva,
            self.scheduler_pointer_rva,
            self.container_offset,
            self.capacity_offset,
            self.serial_counter_offset,
            self.serial_index_offset,
            self.scheduler_pending_offset,
            self.scheduler_ready_offset,
            self.queue_begin_offset,
            self.queue_end_offset,
            self.record_size,
            self.descriptor_size,
            self.capacity,
        )
        return bool(self.dispatch_signature_hex) and all(
            isinstance(value, int) and value >= 0 for value in required
        )

    def require_dispatchable(self) -> None:
        if not self.is_dispatchable():
            raise ValueError(f"Live-add profile {self.profile_id!r} is not dispatchable")

    def resolve_inventory(self, read_u64: Callable[[int], int], module_base: int) -> InventoryPointers:
        return resolve_inventory_pointers(read_u64, module_base, self)


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
    scheduler_ready_offset=0x1629, queue_begin_offset=0x60, queue_end_offset=0x68,
    inventory_global_mode=InventoryGlobalMode.MANAGER_OBJECT.value)


# PC v2.02 candidate layout.  ``accepted`` and ``candidate_only`` keep it out of
# product selection: it is content-complete for review, not dispatchable.  Every
# value in ``PC_V202_EVIDENCE`` is owned by an accepted lane; changing one
# without re-owning its lane is not a version update.
#
# The executable identified by ``PC_V202_EVIDENCE['executable_sha256']`` is the
# source of both captured images.  A candidate still has to clear the remaining
# live steps in docs/knowledge/LIVE_ADD_ENGINEERING.md ("Updating for a new game
# build") before ``accepted`` may become True.
PC_V202_EVIDENCE = {
    'executable_sha256': 'E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130',
    'file_version': '2.0.2.0',
    'text_sha256': '4CEC8FB6AD867417A76DF8201C1D4F54172443884910463C1953ACAD91AE6C29',
    'manager_pointer_rva': 'deepseek-v202-inventory-live-verify + go-v202-acquisition-contract',
    'scheduler_pointer_rva': 'deepseek-v202-scheduler-recovery',
    'scheduler_pending_offset': 'deepseek-v202-scheduler-recovery',
    'scheduler_ready_offset': 'deepseek-v202-scheduler-recovery',
    'queue_begin_offset': 'deepseek-v202-scheduler-recovery',
    'queue_end_offset': 'deepseek-v202-scheduler-recovery',
    'dispatch_rva': 'deepseek-v202-scheduler-recovery',
    'dispatch_return_rva': 'deepseek-v202-scheduler-recovery',
    'dispatch_signature_hex': 'deepseek-v202-scheduler-recovery',
    'builder_rva': 'deepseek-v202-scheduler-recovery',
    'builder_size': 'deepseek-v202-scheduler-recovery',
    'insertion_rva': 'deepseek-v202-scheduler-recovery',
    'insertion_size': 'deepseek-v202-scheduler-recovery',
    'slot_lookup_rva': 'deepseek-v202-layout-static-offsets',
    'container_offset': 'deepseek-v202-layout-static-offsets',
    'capacity_offset': 'deepseek-v202-layout-static-offsets',
    'serial_counter_offset': 'deepseek-v202-layout-static-offsets',
    'serial_index_offset': 'deepseek-v202-layout-static-offsets',
    'inventory_global_mode': 'deepseek-v202-layout-acceptance',
}

PC_V202 = LiveAddProfile(
    profile_id='pc-v2.02-live-add-candidate', file_version=(2, 0, 2, 0),
    dispatch_rva=0x12E9E50, dispatch_return_rva=0x20BB1C,
    dispatch_signature_hex='4053574883EC38',
    builder_rva=0x227FC5C, builder_size=0x27B,
    insertion_rva=0x54D324, insertion_size=0xE17,
    slot_lookup_rva=0x55308C, manager_pointer_rva=0x4751530,
    scheduler_pointer_rva=0x4745348, container_offset=0x224A60,
    capacity_offset=0x16A80, serial_counter_offset=8,
    serial_index_offset=0x23B5E8, scheduler_pending_offset=0x1408,
    scheduler_ready_offset=0x1629, queue_begin_offset=0x60, queue_end_offset=0x68,
    inventory_global_mode=InventoryGlobalMode.MANAGER_OBJECT.value,
    accepted=False, candidate_only=True)


def live_add_profile(version):
    if tuple(version) != PC_V201.file_version or not PC_V201.is_dispatchable():
        raise ValueError('Live insertion has not been accepted for this game version')
    return PC_V201
