"""Offline helpers for Nioh 3 scroll auxiliary generation.

Only behavior recovered from PC v2.00.02 machine code is implemented here.
Every public generator is intentionally narrow and fails closed when a required
native table or row is unavailable.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from functools import lru_cache
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import tempfile
from typing import Any, Callable, Mapping, Sequence

from .r4_finalizer_reference import Lcg32, f32, f32_mul, f32_sub
from .r4_finalizer_resource import (
    R4FinalizerResourceBundle,
    load_default_r4_finalizer_resource,
)
from .r4_table_bundle import FixedStrideTable
from .r4_table_bundle import R4FinalizerTableBundle
from .seed_accelerator import (
    generate_terrain_row_indices_native,
    last_seed_acceleration_backend,
    match_enemy_constraints_native,
    match_special_rule_constraints_native,
)


AUXILIARY_MODE_SEED_MASK_LOW = 0x01E3C78F
AUXILIARY_MODE_SEED_MASK_HIGH = 0x00E1C387
AUXILIARY_MODE_THRESHOLD_KEY = 0x1E7D
AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS = (0x3903, 0x779F, 0x0275)
AUXILIARY_DESCRIPTOR_SELECTOR_KEY = 0xDA38
AUXILIARY_RESOURCE_SCHEMA = "nioh3-auxiliary-generation-resource/v3"
ENEMY_PARAMETER_GATE_CAPTURE_SCHEMA = "nioh3-enemy-parameter-gate-capture/v1"
TERRAIN_DISPLAY_CRUCIBLE_KEY = 0x0024
TERRAIN_DISPLAY_SPECIAL_KEYS = {
    0x2D: 0x0039,
    0xD8: 0x0058,
    0x08: 0x039F,
}
DEFAULT_AUXILIARY_RESOURCE_ROOT = (
    Path(__file__).resolve().parent
    / "data"
    / "auxiliary_generation"
    / "pc_v2_00_02"
    / "resource_v3"
)


class AuxiliaryGenerationError(RuntimeError):
    """Raised when verified native data is insufficient for offline generation."""


@dataclass(frozen=True, slots=True)
class AuxiliaryModeResult:
    """Result of native RVA 0x10291F0 (descriptor byte ``+0x1E``)."""

    value: int
    scoped_seed: int
    branch_class: int
    random_draws: int
    selected_row_index: int | None


@dataclass(frozen=True, slots=True)
class TerrainResult:
    """Result of native RVA 0x1028ED0 (descriptor byte ``+0x1F``)."""

    value: int
    display_effect_keys: tuple[int, ...]
    scoped_seed: int
    used_filtered_pool: bool
    selected_row_index: int
    eligible_row_indices: tuple[int, ...]


@dataclass(frozen=True, slots=True)
class AuxiliaryDescriptorFlagsResult:
    """Result of native RVA 0x1028520 (descriptor bytes ``+0x20..+0x23``)."""

    selector: int
    flags: tuple[bool, bool, bool]
    scoped_seed: int
    random_draws: int


@dataclass(frozen=True, slots=True)
class SpecialRuleResult:
    """Result of native RVA 0x1028880 (three ordered uint16 rule keys)."""

    keys: tuple[int, int, int]
    entries: tuple[SpecialRuleEntryResult, ...]
    scoped_seed: int
    target_budget: int
    random_draws: int


@dataclass(frozen=True, slots=True)
class SpecialRuleEntryResult:
    """UI-consumed value and qualifier semantics for one selected rule row."""

    key: int
    row_index: int
    raw_value: float | None
    display_value: float | None
    display_unit: str | None
    display_grade: str | None
    value_source_offset: int | None
    qualifier_kind: str | None
    qualifier_key: int | None


@dataclass(frozen=True, slots=True)
class EnemyEntryResult:
    """One ordered enemy candidate accepted by a recovered enemy generator."""

    row_index: int
    lookup_key: int
    role: int
    scratch_rule_key: int


@dataclass(frozen=True, slots=True)
class EnemyGroupResult:
    """One displayed enemy group before record/UI name resolution."""

    entries: tuple[EnemyEntryResult, ...]
    source_budget: float


@dataclass(frozen=True, slots=True)
class Class0EnemyResult:
    """Ordered output of native class-0 enemy generation at RVA 0x102AB7A."""

    groups: tuple[EnemyGroupResult, ...]
    scoped_seed: int
    random_draws: int


@dataclass(frozen=True, slots=True)
class Class1EnemyResult:
    """Ordered output of native class-1 enemy generation at RVA 0x102A259."""

    groups: tuple[EnemyGroupResult, ...]
    scoped_seed: int
    random_draws: int


@dataclass(frozen=True, slots=True)
class Class2EnemyResult:
    """Ordered output of native class-2 enemy generation at RVA 0x1029A70."""

    groups: tuple[EnemyGroupResult, ...]
    scoped_seed: int
    random_draws: int


EnemyGenerationResult = Class0EnemyResult | Class1EnemyResult | Class2EnemyResult


@dataclass(frozen=True, slots=True)
class CompleteAuxiliaryResult:
    """Complete deterministic auxiliary output for one displayed Seed."""

    mode: AuxiliaryModeResult
    terrain: TerrainResult
    descriptor: AuxiliaryDescriptorFlagsResult
    enemies: EnemyGenerationResult
    special_rules: SpecialRuleResult


@dataclass(frozen=True, slots=True)
class AuxiliarySearchCriteria:
    """Unordered user constraints for deterministic auxiliary outputs."""

    required_terrain_effect_keys: frozenset[int] = frozenset()
    required_terrain_effect_key_groups: tuple[frozenset[int], ...] = ()
    # Low-level research constraint retained for controlled table-row tests.
    # Product code must filter by the UI-consumed effect keys above.
    terrain_row_indices: frozenset[int] = frozenset()
    required_special_rule_keys: frozenset[int] = frozenset()
    required_special_rule_key_groups: tuple[frozenset[int], ...] = ()
    required_enemy_lookup_keys: frozenset[int] = frozenset()
    required_enemy_lookup_key_groups: tuple[frozenset[int], ...] = ()

    def __post_init__(self) -> None:
        groups = (
            *self.required_terrain_effect_key_groups,
            *self.required_special_rule_key_groups,
            *self.required_enemy_lookup_key_groups,
        )
        if any(not group for group in groups):
            raise ValueError("auxiliary alternative-key groups cannot be empty")
        values = (
            *self.required_terrain_effect_keys,
            *self.required_special_rule_keys,
            *self.required_enemy_lookup_keys,
            *(value for group in groups for value in group),
        )
        if any(not 0 <= value <= 0xFFFFFFFF for value in values):
            raise ValueError("auxiliary keys must fit in uint32")

    @property
    def is_empty(self) -> bool:
        return not (
            self.required_terrain_effect_keys
            or self.required_terrain_effect_key_groups
            or self.terrain_row_indices
            or self.required_special_rule_keys
            or self.required_special_rule_key_groups
            or self.required_enemy_lookup_keys
            or self.required_enemy_lookup_key_groups
        )

    def matches(self, result: CompleteAuxiliaryResult) -> bool:
        return (
            self.matches_terrain(result.terrain)
            and self.matches_special_rules(result.special_rules)
            and self.matches_enemies(result.enemies)
        )

    def matches_terrain(self, terrain: TerrainResult) -> bool:
        if not self.required_terrain_effect_keys.issubset(
            terrain.display_effect_keys
        ):
            return False
        actual_terrain = frozenset(terrain.display_effect_keys)
        if any(
            not group.intersection(actual_terrain)
            for group in self.required_terrain_effect_key_groups
        ):
            return False
        if (
            self.terrain_row_indices
            and terrain.selected_row_index not in self.terrain_row_indices
        ):
            return False
        return True

    def matches_special_rules(self, special_rules: SpecialRuleResult) -> bool:
        actual_rules = frozenset(key for key in special_rules.keys if key)
        if not self.required_special_rule_keys.issubset(actual_rules):
            return False
        if any(
            not group.intersection(actual_rules)
            for group in self.required_special_rule_key_groups
        ):
            return False
        return True

    def matches_enemies(self, enemies: EnemyGenerationResult) -> bool:
        actual_enemies = frozenset(
            entry.lookup_key
            for group in enemies.groups
            for entry in group.entries
        )
        if not self.required_enemy_lookup_keys.issubset(actual_enemies):
            return False
        return all(
            group.intersection(actual_enemies)
            for group in self.required_enemy_lookup_key_groups
        )


@dataclass(frozen=True, slots=True)
class AuxiliaryGenerationTables:
    """Pointer-free auxiliary tables loaded from a verified runtime capture."""

    terrain: FixedStrideTable
    terrain_keys_by_row: tuple[int, ...]
    special_rules: FixedStrideTable | None = None
    special_rule_keys_by_row: tuple[int, ...] = ()
    rule_conflicts: FixedStrideTable | None = None
    rule_conflict_keys_by_row: tuple[int, ...] = ()
    enemy_candidates: FixedStrideTable | None = None
    special_context: FixedStrideTable | None = None
    enemy_param_type_by_key: Mapping[int, int] = field(default_factory=dict)

    @classmethod
    def from_runtime_capture(
        cls,
        root: str | Path,
        *,
        enemy_gate_capture_root: str | Path | None = None,
        verify: bool = True,
    ) -> "AuxiliaryGenerationTables":
        capture = R4FinalizerTableBundle(root, verify=verify)
        terrain = capture.table("auxiliary_terrain")
        keys = _hash_keys_by_row(
            capture.root,
            capture.manifest,
            table_name="auxiliary_terrain",
            row_count=terrain.row_count,
        )
        special_rules = capture.table("scroll_special_rule")
        special_rule_keys = _hash_keys_by_row(
            capture.root,
            capture.manifest,
            table_name="scroll_special_rule",
            row_count=special_rules.row_count,
        )
        rule_conflicts = capture.table("auxiliary_rule_conflict")
        rule_conflict_keys = _hash_keys_by_row(
            capture.root,
            capture.manifest,
            table_name="auxiliary_rule_conflict",
            row_count=rule_conflicts.row_count,
        )
        enemy_candidates = capture.table("auxiliary_enemy_candidate")
        special_context = capture.table("special_context")
        return cls(
            terrain=terrain,
            terrain_keys_by_row=keys,
            special_rules=special_rules,
            special_rule_keys_by_row=special_rule_keys,
            rule_conflicts=rule_conflicts,
            rule_conflict_keys_by_row=rule_conflict_keys,
            enemy_candidates=enemy_candidates,
            special_context=special_context,
            enemy_param_type_by_key=(
                load_enemy_parameter_gate_capture(enemy_gate_capture_root)
                if enemy_gate_capture_root is not None
                else {}
            ),
        )

    @classmethod
    def from_resource(
        cls, root: str | Path, *, verify: bool = True
    ) -> "AuxiliaryGenerationTables":
        resource_root = Path(root)
        manifest_path = resource_root / "manifest.json"
        manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
        if manifest.get("schema") != AUXILIARY_RESOURCE_SCHEMA:
            raise AuxiliaryGenerationError(
                f"unsupported auxiliary resource schema: {manifest.get('schema')!r}"
            )

        def load_raw_table(name: str) -> FixedStrideTable:
            table_meta = manifest["tables"][name]
            if verify:
                table_data = _read_declared_blob(resource_root, table_meta["file"])
            else:
                table_data = _safe_capture_path(
                    resource_root, table_meta["file"]["filename"]
                ).read_bytes()
            row_count = int(table_meta["row_count"])
            return FixedStrideTable(
                name,
                int(table_meta["row_size"]),
                row_count,
                table_data,
            )

        def load_table(name: str) -> tuple[FixedStrideTable, tuple[int, ...]]:
            table_meta = manifest["tables"][name]
            table = load_raw_table(name)
            if verify:
                key_data = _read_declared_blob(resource_root, table_meta["keys_file"])
            else:
                key_data = _safe_capture_path(
                    resource_root, table_meta["keys_file"]["filename"]
                ).read_bytes()
            row_count = table.row_count
            if len(key_data) != row_count * 2:
                raise AuxiliaryGenerationError(f"{name}: key resource size mismatch")
            keys = struct.unpack(f"<{row_count}H", key_data) if row_count else ()
            return table, tuple(keys)

        gate_meta = manifest.get("enemy_parameter_gate")
        if not isinstance(gate_meta, dict):
            raise AuxiliaryGenerationError("resource has no enemy parameter gate")
        if verify:
            gate_data = _read_declared_blob(resource_root, gate_meta["file"])
        else:
            gate_data = _safe_capture_path(
                resource_root, gate_meta["file"]["filename"]
            ).read_bytes()
        entry_count = int(gate_meta["entry_count"])
        if len(gate_data) != entry_count * 8:
            raise AuxiliaryGenerationError("enemy parameter gate size mismatch")
        enemy_parameter_gate: dict[int, int] = {}
        for offset in range(0, len(gate_data), 8):
            key, parameter_type = struct.unpack_from("<II", gate_data, offset)
            if key in enemy_parameter_gate:
                raise AuxiliaryGenerationError(
                    f"duplicate enemy parameter gate key 0x{key:08X}"
                )
            enemy_parameter_gate[key] = parameter_type

        terrain, terrain_keys = load_table("auxiliary_terrain")
        special_rules, special_rule_keys = load_table("scroll_special_rule")
        rule_conflicts, rule_conflict_keys = load_table("auxiliary_rule_conflict")
        enemy_candidates = load_raw_table("auxiliary_enemy_candidate")
        special_context = load_raw_table("special_context")
        return cls(
            terrain=terrain,
            terrain_keys_by_row=terrain_keys,
            special_rules=special_rules,
            special_rule_keys_by_row=special_rule_keys,
            rule_conflicts=rule_conflicts,
            rule_conflict_keys_by_row=rule_conflict_keys,
            enemy_candidates=enemy_candidates,
            special_context=special_context,
            enemy_param_type_by_key=enemy_parameter_gate,
        )


def _file_metadata(relative: str, data: bytes) -> dict[str, object]:
    return {
        "filename": relative.replace(os.sep, "/"),
        "size": len(data),
        "sha256": hashlib.sha256(data).hexdigest().upper(),
    }


def build_auxiliary_generation_resource(
    capture_root: str | Path,
    enemy_gate_capture_root: str | Path,
    output_root: str | Path,
) -> Path:
    """Derive a minimal pointer-free resource from a verified runtime capture."""

    capture_path = Path(capture_root)
    output_path = Path(output_root)
    if output_path.exists():
        raise FileExistsError(f"refusing to overwrite existing output: {output_path}")

    source_bundle = R4FinalizerTableBundle(capture_path, verify=True)
    gate_capture_path = Path(enemy_gate_capture_root)
    tables = AuxiliaryGenerationTables.from_runtime_capture(
        capture_path,
        enemy_gate_capture_root=gate_capture_path,
        verify=True,
    )
    source_manifest_bytes = source_bundle.manifest_path.read_bytes()
    table_payloads = {
        "auxiliary_terrain": (tables.terrain, tables.terrain_keys_by_row),
        "scroll_special_rule": (tables.special_rules, tables.special_rule_keys_by_row),
        "auxiliary_rule_conflict": (tables.rule_conflicts, tables.rule_conflict_keys_by_row),
    }
    if any(table is None for table, _ in table_payloads.values()):
        raise AuxiliaryGenerationError("capture is missing required auxiliary tables")
    if tables.enemy_candidates is None or tables.special_context is None:
        raise AuxiliaryGenerationError("capture is missing enemy generation tables")
    if not tables.enemy_param_type_by_key:
        raise AuxiliaryGenerationError("capture is missing the enemy parameter gate")

    output_path.parent.mkdir(parents=True, exist_ok=True)
    staging_parent = Path(
        tempfile.mkdtemp(prefix=f".{output_path.name}.tmp-", dir=output_path.parent)
    )
    staging = staging_parent / "resource"
    try:
        (staging / "tables").mkdir(parents=True)
        table_manifest: dict[str, object] = {}
        for name, (table, keys) in table_payloads.items():
            assert table is not None
            table_data = table.row_store
            key_data = struct.pack(f"<{len(keys)}H", *keys)
            table_relative = f"tables/{name}.bin"
            keys_relative = f"tables/{name}_keys.bin"
            (staging / table_relative).write_bytes(table_data)
            (staging / keys_relative).write_bytes(key_data)
            table_manifest[name] = {
                "row_size": table.row_size,
                "row_count": table.row_count,
                "file": _file_metadata(table_relative, table_data),
                "keys_file": _file_metadata(keys_relative, key_data),
            }
        for name, table in (
            ("auxiliary_enemy_candidate", tables.enemy_candidates),
            ("special_context", tables.special_context),
        ):
            table_data = table.row_store
            table_relative = f"tables/{name}.bin"
            (staging / table_relative).write_bytes(table_data)
            table_manifest[name] = {
                "row_size": table.row_size,
                "row_count": table.row_count,
                "file": _file_metadata(table_relative, table_data),
            }
        gate_data = b"".join(
            struct.pack("<II", key, parameter_type)
            for key, parameter_type in sorted(tables.enemy_param_type_by_key.items())
        )
        gate_relative = "tables/enemy_parameter_gate.bin"
        (staging / gate_relative).write_bytes(gate_data)
        manifest = {
            "schema": AUXILIARY_RESOURCE_SCHEMA,
            "game_version": "PC v2.00.02",
            "source": {
                "capture_schema": source_bundle.manifest.get("schema"),
                "capture_manifest_sha256": hashlib.sha256(
                    source_manifest_bytes
                ).hexdigest().upper(),
                "effective_playthrough": source_bundle.effective_playthrough,
                "enemy_gate_capture_manifest_sha256": hashlib.sha256(
                    (gate_capture_path / "manifest.json").read_bytes()
                ).hexdigest().upper(),
            },
            "tables": table_manifest,
            "enemy_parameter_gate": {
                "entry_count": len(tables.enemy_param_type_by_key),
                "file": _file_metadata(gate_relative, gate_data),
            },
            "safety": {
                "process_specific_metadata_omitted": True,
                "runtime_pointers_omitted": True,
            },
        }
        (staging / "manifest.json").write_text(
            json.dumps(manifest, ensure_ascii=False, indent=2) + "\n",
            encoding="utf-8",
        )
        AuxiliaryGenerationTables.from_resource(staging, verify=True)
        staging.rename(output_path)
        staging_parent.rmdir()
        return output_path
    except Exception:
        shutil.rmtree(staging_parent, ignore_errors=True)
        raise


@lru_cache(maxsize=2)
def load_default_auxiliary_generation_tables(
    *, verify: bool = True
) -> AuxiliaryGenerationTables:
    return AuxiliaryGenerationTables.from_resource(
        DEFAULT_AUXILIARY_RESOURCE_ROOT, verify=verify
    )


def derive_auxiliary_mode_seed(displayed_seed: int) -> int:
    """Reproduce the scoped RNG seed installed at RVA 0x1029204..0x102921E."""

    seed = displayed_seed & 0xFFFFFFFF
    return (
        ((seed & AUXILIARY_MODE_SEED_MASK_LOW) << 3)
        | ((seed >> 4) & AUXILIARY_MODE_SEED_MASK_HIGH)
    ) & 0xFFFFFFFF


def derive_terrain_seed(displayed_seed: int) -> int:
    """Swap the two low 14-bit Seed halves used by RVA 0x1028EEA..0x1028F08."""

    seed = displayed_seed & 0xFFFFFFFF
    return (((seed >> 14) & 0x3FFF) | ((seed & 0x3FFF) << 14)) & 0xFFFFFFFF


def derive_special_rule_seed(displayed_seed: int) -> int:
    """Reproduce the 28-bit scoped seed installed by RVA 0x10288A3..0x10288F2."""

    return displayed_seed & 0x0FFFFFFF


def derive_auxiliary_descriptor_seed(displayed_seed: int) -> int:
    """Reproduce the scoped seed installed by RVA 0x102853C..0x1028567."""

    seed = displayed_seed & 0xFFFFFFFF
    return (
        ((seed & AUXILIARY_MODE_SEED_MASK_HIGH) << 4)
        | ((seed >> 3) & AUXILIARY_MODE_SEED_MASK_LOW)
    ) & 0xFFFFFFFF


def _safe_capture_path(root: Path, value: object) -> Path:
    relative = Path(str(value))
    if relative.is_absolute() or ".." in relative.parts:
        raise AuxiliaryGenerationError(f"unsafe capture path: {relative}")
    return root / relative


def _read_declared_blob(root: Path, metadata: dict[str, Any]) -> bytes:
    path = _safe_capture_path(root, metadata["filename"])
    data = path.read_bytes()
    if len(data) != int(metadata["size"]):
        raise AuxiliaryGenerationError(f"capture size mismatch: {path}")
    digest = hashlib.sha256(data).hexdigest().upper()
    if digest != str(metadata["sha256"]).upper():
        raise AuxiliaryGenerationError(f"capture hash mismatch: {path}")
    return data


def _hash_keys_by_row(
    root: Path,
    manifest: dict[str, Any],
    *,
    table_name: str,
    row_count: int,
) -> tuple[int, ...]:
    table_metadata = {
        str(item["name"]): item for item in manifest.get("tables", [])
    }.get(table_name)
    if table_metadata is None:
        raise AuxiliaryGenerationError(f"capture has no table {table_name!r}")
    hash_metadata = table_metadata.get("hash")
    if not isinstance(hash_metadata, dict) or not hash_metadata.get("available"):
        raise AuxiliaryGenerationError(f"{table_name}: native hash index is unavailable")

    context = _read_declared_blob(root, hash_metadata["context_blob"])
    entries = _read_declared_blob(root, hash_metadata["entries_blob"])
    if len(context) < 6 or len(entries) % 8:
        raise AuxiliaryGenerationError(f"{table_name}: malformed native hash data")
    sentinel = struct.unpack_from("<H", context, 4)[0]
    keys: list[int | None] = [None] * row_count
    for offset in range(0, len(entries), 8):
        key = struct.unpack_from("<H", entries, offset)[0]
        row_index = struct.unpack_from("<I", entries, offset + 4)[0]
        if key == sentinel or row_index >= row_count:
            continue
        if keys[row_index] is None:
            keys[row_index] = key
        elif keys[row_index] != key:
            raise AuxiliaryGenerationError(
                f"{table_name}: row {row_index} has multiple native keys"
            )
    missing = [index for index, key in enumerate(keys) if key is None]
    if missing:
        raise AuxiliaryGenerationError(
            f"{table_name}: hash index does not cover rows {missing[:8]}"
        )
    return tuple(int(key) for key in keys)


def load_enemy_parameter_gate_capture(
    root: str | Path,
    *,
    verify: bool = True,
) -> dict[int, int]:
    """Load the complete lookup-key to ``enemy row +0x80`` gate mapping."""

    capture_root = Path(root)
    manifest_path = capture_root / "manifest.json"
    manifest = json.loads(manifest_path.read_text(encoding="utf-8"))
    if manifest.get("schema") != ENEMY_PARAMETER_GATE_CAPTURE_SCHEMA:
        raise AuxiliaryGenerationError(
            f"unsupported enemy gate capture schema: {manifest.get('schema')!r}"
        )
    table_metadata = {
        str(item["name"]): item for item in manifest.get("tables", [])
    }.get("enemy_parameter")
    if table_metadata is None:
        raise AuxiliaryGenerationError("capture has no enemy_parameter table")
    if int(table_metadata["row_size"]) != 0x398:
        raise AuxiliaryGenerationError("enemy parameter row size is not 0x398")

    def read_blob(metadata: dict[str, Any]) -> bytes:
        if verify:
            return _read_declared_blob(capture_root, metadata)
        return _safe_capture_path(capture_root, metadata["filename"]).read_bytes()

    rows_data = read_blob(table_metadata["rows_blob"])
    row_count = int(table_metadata["row_count"])
    rows = FixedStrideTable("enemy_parameter", 0x398, row_count, rows_data)
    hash_metadata = table_metadata.get("hash")
    if not isinstance(hash_metadata, dict) or not hash_metadata.get("available"):
        raise AuxiliaryGenerationError("enemy parameter native hash index is unavailable")
    context = read_blob(hash_metadata["context_blob"])
    entries = read_blob(hash_metadata["entries_blob"])
    if len(context) < 8 or len(entries) % 8:
        raise AuxiliaryGenerationError("enemy parameter native hash data is malformed")

    sentinel = struct.unpack_from("<I", context, 4)[0]
    result: dict[int, int] = {}
    for offset in range(0, len(entries), 8):
        key, row_index = struct.unpack_from("<II", entries, offset)
        if key == sentinel:
            continue
        if row_index >= row_count:
            raise AuxiliaryGenerationError(
                f"enemy parameter hash row {row_index} exceeds {row_count} rows"
            )
        parameter_type = struct.unpack_from("<I", rows.row(row_index), 0x80)[0]
        previous = result.setdefault(key, parameter_type)
        if previous != parameter_type:
            raise AuxiliaryGenerationError(
                f"enemy lookup key 0x{key:08X} has conflicting gate types"
            )
    if not result:
        raise AuxiliaryGenerationError("enemy parameter native hash index is empty")
    return result


@lru_cache(maxsize=128)
def _find_optional_multiplier_row(
    table: FixedStrideTable, key: int
) -> tuple[int, bytes]:
    # The key consumed by RVA 0x645DB0 is stored at row +0x14 for this 0x20-byte
    # table. The same layout is used by the existing R4 weight implementation.
    matches = table.find_u32(key, offset=0x14)
    if len(matches) != 1:
        raise AuxiliaryGenerationError(
            f"optional multiplier key 0x{key:04X} resolved to {len(matches)} rows"
        )
    index = matches[0]
    return index, table.row(index)


def _optional_multiplier_threshold(table: FixedStrideTable, key: int) -> int:
    """Resolve the integer threshold used by the native 0..9999 lottery."""

    _, row = _find_optional_multiplier_row(table, key)
    base = struct.unpack_from("<i", row, 0x10)[0]
    scale = struct.unpack_from("<f", row, 0x18)[0]
    return int(f32_mul(base, scale))


def _roll_10000(rng: Lcg32) -> int:
    return min(int(f32_mul(rng.next_float01(), f32(10000.0))), 9999)


def generate_auxiliary_descriptor_flags(
    displayed_seed: int,
    auxiliary_mode: int,
    *,
    resource: R4FinalizerResourceBundle | None = None,
    tables: AuxiliaryGenerationTables | None = None,
) -> AuxiliaryDescriptorFlagsResult:
    """Generate descriptor bytes ``+0x20..+0x23`` from native RVA 0x1028520.

    The first three output flags are exact pointer-free parity. The uncommon
    selector path chooses one value from a native linked list. The linked-list
    order was not captured, but the complete enemy-candidate table contains
    exactly one nonzero selector value in PC v2.00.02, so every possible list
    choice has the same externally observable result. This implementation
    derives that unique value from the verified pointer-free table and fails
    closed if a future table contains more than one value.
    """

    if not 0 <= auxiliary_mode <= 0xFF:
        raise ValueError("auxiliary_mode must fit in one byte")
    if resource is None:
        resource = load_default_r4_finalizer_resource()

    optional_table = resource.table("optional_multiplier")
    scoped_seed = derive_auxiliary_descriptor_seed(displayed_seed)
    rng = Lcg32(scoped_seed)
    flags = tuple(
        _optional_multiplier_threshold(optional_table, key) > _roll_10000(rng)
        for key in AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS
    )
    draws = 3

    context_rows = [
        row
        for row in resource.table("special_context").rows()
        if row[0x28] == auxiliary_mode
    ]
    if len(context_rows) > 1:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} resolved to multiple context rows"
        )

    selector = 0
    if context_rows and context_rows[0][0x29] != 0:
        selector_roll = _roll_10000(rng)
        draws += 1
        selector_threshold = _optional_multiplier_threshold(
            optional_table,
            AUXILIARY_DESCRIPTOR_SELECTOR_KEY,
        )
        if selector_threshold > selector_roll:
            if tables is None:
                tables = load_default_auxiliary_generation_tables()
            if tables.enemy_candidates is None:
                raise AuxiliaryGenerationError(
                    "auxiliary descriptor selector requires the enemy-candidate table"
                )
            selector_values = frozenset(
                row[0x19] for row in tables.enemy_candidates.rows() if row[0x19]
            )
            if len(selector_values) != 1:
                raise AuxiliaryGenerationError(
                    "auxiliary descriptor selector linked-list order is required "
                    f"for {len(selector_values)} distinct table values"
                )
            selector = next(iter(selector_values))

    return AuxiliaryDescriptorFlagsResult(
        selector=selector,
        flags=(bool(flags[0]), bool(flags[1]), bool(flags[2])),
        scoped_seed=scoped_seed,
        random_draws=draws,
    )


def generate_auxiliary_mode(
    displayed_seed: int,
    *,
    resource: R4FinalizerResourceBundle | None = None,
) -> AuxiliaryModeResult:
    """Generate descriptor byte ``+0x1E`` exactly as native RVA 0x10291F0.

    Native behavior uses the optional-multiplier row keyed by ``0x1E7D`` and
    the seven rows at parameter-manager ``+0xA98``. No game process is needed
    because both tables are already bundled as pointer-free v2.00.02 data.
    """

    if resource is None:
        resource = load_default_r4_finalizer_resource()

    optional_table = resource.table("optional_multiplier")
    context_table = resource.table("special_context")
    _, threshold_row = _find_optional_multiplier_row(
        optional_table, AUXILIARY_MODE_THRESHOLD_KEY
    )

    threshold_base = struct.unpack_from("<i", threshold_row, 0x10)[0]
    threshold_scale = struct.unpack_from("<f", threshold_row, 0x18)[0]
    threshold = int(f32_mul(threshold_base, threshold_scale))

    scoped_seed = derive_auxiliary_mode_seed(displayed_seed)
    rng = Lcg32(scoped_seed)
    first_roll = min(int(f32_mul(rng.next_float01(), f32(10000.0))), 9999)
    draws = 1

    # EDI starts at 2. When the first roll misses the threshold, native code
    # consumes another draw and splits the remaining path evenly into classes
    # 1 and 0 (RVA 0x102928C..0x10292A1).
    branch_class = 2
    if first_roll >= threshold:
        second_roll = int(f32_mul(rng.next_float01(), f32(2.0)))
        draws += 1
        branch_class = 1 if second_roll == 0 else 0

    matching_indices = [
        index
        for index, row in enumerate(context_table.rows())
        if row[0x29] == branch_class
    ]
    if not matching_indices:
        return AuxiliaryModeResult(
            value=0,
            scoped_seed=scoped_seed,
            branch_class=branch_class,
            random_draws=draws,
            selected_row_index=None,
        )

    selected = rng.random_int(len(matching_indices))
    draws += 1
    row_index = matching_indices[selected]
    value = context_table.row(row_index)[0x28]
    return AuxiliaryModeResult(
        value=value,
        scoped_seed=scoped_seed,
        branch_class=branch_class,
        random_draws=draws,
        selected_row_index=row_index,
    )


def generate_terrain(
    displayed_seed: int,
    auxiliary_mode: int,
    *,
    tables: AuxiliaryGenerationTables,
    resource: R4FinalizerResourceBundle | None = None,
) -> TerrainResult:
    """Generate descriptor byte ``+0x1F`` exactly as native RVA 0x1028ED0."""

    if not 0 <= auxiliary_mode <= 0xFF:
        raise ValueError("auxiliary_mode must fit in one byte")
    if resource is None:
        resource = load_default_r4_finalizer_resource()

    context_table = resource.table("special_context")
    context_matches = [
        row
        for row in context_table.rows()
        if row[0x28] == auxiliary_mode
    ]
    if len(context_matches) > 1:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} resolved to multiple context rows"
        )

    terrain = tables.terrain
    if terrain.row_count == 0:
        raise AuxiliaryGenerationError("native terrain table is empty")
    if len(tables.terrain_keys_by_row) != terrain.row_count:
        raise AuxiliaryGenerationError("terrain key index does not match row count")

    scoped_seed = derive_terrain_seed(displayed_seed)
    rng = Lcg32(scoped_seed)
    use_filtered_pool = bool(context_matches and context_matches[0][0x29] != 2)
    if use_filtered_pool:
        eligible = tuple(
            index
            for index, row in enumerate(terrain.rows())
            if not (row[0x2E] & 0x02)
        )
        if not eligible:
            raise AuxiliaryGenerationError("native filtered terrain pool is empty")
        selected_row = eligible[rng.random_int(len(eligible))]
        value = terrain.row(selected_row)[0x30]
    else:
        eligible = tuple(range(terrain.row_count))
        selected_row = rng.random_int(terrain.row_count)
        value = tables.terrain_keys_by_row[selected_row] & 0xFF

    selected_row_data = terrain.row(selected_row)
    display_effect_keys: list[int] = []
    # Scroll-detail consumer RVA 0x1F0E092 looks the generated enum up in the
    # same +0xA88 table. A nonzero u16 at row +0x2C emits terrain-effect key
    # 0x24 (The Crucible). Three enum values append a second hard-coded effect.
    if struct.unpack_from("<H", selected_row_data, 0x2C)[0] != 0:
        display_effect_keys.append(TERRAIN_DISPLAY_CRUCIBLE_KEY)
    special_key = TERRAIN_DISPLAY_SPECIAL_KEYS.get(value)
    if special_key is not None:
        display_effect_keys.append(special_key)

    return TerrainResult(
        value=value,
        display_effect_keys=tuple(display_effect_keys),
        scoped_seed=scoped_seed,
        used_filtered_pool=use_filtered_pool,
        selected_row_index=selected_row,
        eligible_row_indices=eligible,
    )


@lru_cache(maxsize=2)
def _terrain_batch_configuration(
    *,
    verify: bool = True,
) -> tuple[int, tuple[int, ...], int]:
    """Return immutable native inputs for batched terrain-row generation."""

    tables = load_default_auxiliary_generation_tables(verify=verify)
    resource = load_default_r4_finalizer_resource()
    optional_table = resource.table("optional_multiplier")
    _, threshold_row = _find_optional_multiplier_row(
        optional_table,
        AUXILIARY_MODE_THRESHOLD_KEY,
    )
    threshold_base = struct.unpack_from("<i", threshold_row, 0x10)[0]
    threshold_scale = struct.unpack_from("<f", threshold_row, 0x18)[0]
    threshold = int(f32_mul(threshold_base, threshold_scale))
    filtered_rows = tuple(
        index
        for index, row in enumerate(tables.terrain.rows())
        if not (row[0x2E] & 0x02)
    )
    if not filtered_rows:
        raise AuxiliaryGenerationError("native filtered terrain pool is empty")
    for row_index, row in enumerate(tables.terrain.rows()):
        if row[0x30] != (tables.terrain_keys_by_row[row_index] & 0xFF):
            raise AuxiliaryGenerationError(
                "terrain row value differs between filtered and full native pools"
            )
    return threshold, filtered_rows, tables.terrain.row_count


def generate_terrain_row_indices_batch(
    displayed_seeds: Sequence[int],
    *,
    tables: AuxiliaryGenerationTables | None = None,
    resource: R4FinalizerResourceBundle | None = None,
    require_cuda: bool = False,
) -> tuple[int, ...]:
    """Generate exact terrain row indices in a native/CUDA batch when possible."""

    seeds = tuple(int(seed) & 0xFFFFFFFF for seed in displayed_seeds)
    if not seeds:
        return ()
    if tables is None and resource is None:
        threshold, filtered_rows, terrain_row_count = _terrain_batch_configuration()
        native = generate_terrain_row_indices_native(
            seeds,
            mode_threshold=threshold,
            filtered_row_indices=filtered_rows,
            terrain_row_count=terrain_row_count,
        )
        if native is not None:
            if require_cuda and last_seed_acceleration_backend() != "cuda":
                raise RuntimeError(
                    "CUDA terrain matcher is unavailable; CPU fallback is disabled"
                )
            return native
        if require_cuda:
            raise RuntimeError(
                "CUDA terrain matcher is unavailable; CPU fallback is disabled"
            )
        tables = load_default_auxiliary_generation_tables()
        resource = load_default_r4_finalizer_resource()
    else:
        if tables is None:
            tables = load_default_auxiliary_generation_tables()
        if resource is None:
            resource = load_default_r4_finalizer_resource()

    return tuple(
        generate_terrain(
            seed,
            generate_auxiliary_mode(seed, resource=resource).value,
            tables=tables,
            resource=resource,
        ).selected_row_index
        for seed in seeds
    )


def terrain_row_matches_criteria(
    row_index: int,
    criteria: AuxiliarySearchCriteria,
    *,
    tables: AuxiliaryGenerationTables | None = None,
) -> bool:
    """Apply only UI-visible terrain constraints to a prefetched native row."""

    actual = terrain_display_effect_keys_for_row(row_index, tables=tables)
    if not criteria.required_terrain_effect_keys.issubset(actual):
        return False
    if any(
        not group.intersection(actual)
        for group in criteria.required_terrain_effect_key_groups
    ):
        return False
    return not criteria.terrain_row_indices or row_index in criteria.terrain_row_indices


def terrain_display_effect_keys_for_row(
    row_index: int,
    *,
    tables: AuxiliaryGenerationTables | None = None,
) -> frozenset[int]:
    """Return every player-visible terrain effect emitted by one native row."""

    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    if not 0 <= row_index < tables.terrain.row_count:
        raise IndexError(row_index)
    row = tables.terrain.row(row_index)
    display_effect_keys: list[int] = []
    if struct.unpack_from("<H", row, 0x2C)[0] != 0:
        display_effect_keys.append(TERRAIN_DISPLAY_CRUCIBLE_KEY)
    value = tables.terrain_keys_by_row[row_index] & 0xFF
    special_key = TERRAIN_DISPLAY_SPECIAL_KEYS.get(value)
    if special_key is not None:
        display_effect_keys.append(special_key)
    return frozenset(display_effect_keys)


def terrain_rows_containing_effects(
    required_effect_keys: frozenset[int],
    *,
    tables: AuxiliaryGenerationTables | None = None,
) -> tuple[int, ...]:
    """Return native terrain rows containing all requested display effects."""

    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    return tuple(
        row_index
        for row_index in range(tables.terrain.row_count)
        if required_effect_keys.issubset(
            terrain_display_effect_keys_for_row(row_index, tables=tables)
        )
    )


@lru_cache(maxsize=1)
def _enemy_batch_configuration() -> tuple[
    int,
    tuple[int, int, int],
    int,
    int,
    int,
    bytes,
    bytes,
    bytes,
]:
    """Pack immutable PC v2.00.02 enemy inputs for the native matcher."""

    tables = load_default_auxiliary_generation_tables()
    resource = load_default_r4_finalizer_resource()
    if tables.enemy_candidates is None or tables.special_context is None:
        raise AuxiliaryGenerationError("enemy generation tables are unavailable")
    optional = resource.table("optional_multiplier")
    mode_threshold = _optional_multiplier_threshold(
        optional,
        AUXILIARY_MODE_THRESHOLD_KEY,
    )
    descriptor_thresholds = tuple(
        _optional_multiplier_threshold(optional, key)
        for key in AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS
    )
    selector_threshold = _optional_multiplier_threshold(
        optional,
        AUXILIARY_DESCRIPTOR_SELECTOR_KEY,
    )
    role_five_threshold = _optional_multiplier_threshold(optional, 0xCEFC)
    selector_values = frozenset(
        row[0x19] for row in tables.enemy_candidates.rows() if row[0x19]
    )
    if len(selector_values) != 1:
        raise AuxiliaryGenerationError(
            "native enemy matcher requires one stable descriptor selector"
        )
    selector_value = next(iter(selector_values))
    packed_enemy_rows = bytearray()
    for row in tables.enemy_candidates.rows():
        lookup_key = struct.unpack_from("<I", row, 0x04)[0]
        packed_enemy_rows.extend(
            struct.pack(
                "<IfHH6B",
                lookup_key,
                struct.unpack_from("<f", row, 0x0C)[0],
                struct.unpack_from("<H", row, 0x12)[0],
                struct.unpack_from("<H", row, 0x14)[0],
                row[0x16],
                row[0x18],
                row[0x19],
                row[0x1A],
                row[0x1B],
                tables.enemy_param_type_by_key.get(lookup_key, 0xFF),
            )
        )
    packed_terrains = b"".join(
        struct.pack(
            "<HHB",
            struct.unpack_from("<H", row, 0x2C)[0],
            struct.unpack_from("<H", row, 0x2E)[0],
            row[0x31],
        )
        for row in tables.terrain.rows()
    )
    packed_contexts = b"".join(
        struct.pack(
            "<BB5f",
            row[0x28],
            row[0x29],
            *struct.unpack_from("<5f", row, 0x04),
        )
        for row in tables.special_context.rows()
    )
    return (
        mode_threshold,
        (
            int(descriptor_thresholds[0]),
            int(descriptor_thresholds[1]),
            int(descriptor_thresholds[2]),
        ),
        selector_threshold,
        role_five_threshold,
        selector_value,
        bytes(packed_enemy_rows),
        packed_terrains,
        packed_contexts,
    )


def generate_enemy_match_masks_batch(
    displayed_seeds: Sequence[int],
    terrain_row_indices: Sequence[int],
    playthrough: int,
    *,
    criteria: AuxiliarySearchCriteria,
    require_cuda: bool = False,
) -> tuple[int, ...]:
    """Return one bit mask for every requested enemy condition group."""

    seeds = tuple(int(seed) & 0xFFFFFFFF for seed in displayed_seeds)
    terrain_rows = tuple(int(row) for row in terrain_row_indices)
    if len(seeds) != len(terrain_rows):
        raise ValueError("enemy matcher requires one terrain row per Seed")
    groups = tuple(
        (frozenset((key,)) for key in sorted(criteria.required_enemy_lookup_keys))
    ) + tuple(criteria.required_enemy_lookup_key_groups)
    if not groups:
        return (0,) * len(seeds)
    if not seeds:
        return ()
    (
        mode_threshold,
        descriptor_thresholds,
        selector_threshold,
        role_five_threshold,
        selector_value,
        packed_enemy_rows,
        packed_terrains,
        packed_contexts,
    ) = _enemy_batch_configuration()
    native = match_enemy_constraints_native(
        seeds,
        terrain_rows,
        playthrough=playthrough,
        mode_threshold=mode_threshold,
        descriptor_thresholds=descriptor_thresholds,
        selector_threshold=selector_threshold,
        role_five_threshold=role_five_threshold,
        selector_value=selector_value,
        enemy_rows=packed_enemy_rows,
        terrains=packed_terrains,
        contexts=packed_contexts,
        criterion_groups=groups,
    )
    if native is not None:
        if require_cuda and last_seed_acceleration_backend() != "cuda":
            raise RuntimeError(
                "CUDA enemy matcher is unavailable; CPU fallback is disabled"
            )
        return native
    if require_cuda:
        raise RuntimeError(
            "CUDA enemy matcher is unavailable; CPU fallback is disabled"
        )

    output: list[int] = []
    for seed in seeds:
        auxiliary = generate_complete_auxiliary(seed, playthrough)
        actual = frozenset(
            entry.lookup_key
            for group in auxiliary.enemies.groups
            for entry in group.entries
        )
        mask = 0
        for index, group in enumerate(groups):
            if group.intersection(actual):
                mask |= 1 << index
        output.append(mask)
    return tuple(output)


@lru_cache(maxsize=5)
def _special_rule_batch_configuration(
    playthrough: int,
) -> tuple[tuple[int, ...], tuple[frozenset[int], ...], bytes]:
    """Pack exact rule rows and enemy scratch-key groups for one playthrough."""

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    tables = load_default_auxiliary_generation_tables()
    if (
        tables.enemy_candidates is None
        or tables.special_rules is None
        or tables.rule_conflicts is None
    ):
        raise AuxiliaryGenerationError("special-rule generation tables are unavailable")
    conflict_rows = dict(
        zip(
            tables.rule_conflict_keys_by_row,
            tables.rule_conflicts.rows(),
            strict=True,
        )
    )
    enemy_rows = tuple(tables.enemy_candidates.rows())
    scratch_keys = tuple(
        sorted(
            {
                struct.unpack_from("<H", row, 0x12)[0]
                for row in enemy_rows
                if struct.unpack_from("<H", row, 0x12)[0] != 0xFFFF
            }
        )
    )
    if len(scratch_keys) > 32:
        raise AuxiliaryGenerationError(
            "native special-rule matcher supports at most 32 scratch keys"
        )
    enemy_groups = tuple(
        frozenset(
            struct.unpack_from("<I", row, 0x04)[0]
            for row in enemy_rows
            if struct.unpack_from("<H", row, 0x12)[0] == scratch_key
        )
        for scratch_key in scratch_keys
    )
    scratch_bit_by_key = {
        key: bit for bit, key in enumerate(scratch_keys)
    }
    packed = bytearray()
    for key, row in zip(
        tables.special_rule_keys_by_row,
        tables.special_rules.rows(),
        strict=True,
    ):
        identities: list[int] = []
        active_mask = 0
        for index, group_key in enumerate(struct.unpack_from("<HH", row, 0x2C)):
            conflict = conflict_rows.get(group_key)
            if conflict is None:
                identities.append(0xFFFF)
                continue
            identities.append(struct.unpack_from("<H", conflict, 0x08)[0])
            if conflict[0x0C] & 0x01:
                active_mask |= 1 << index
        weight = _rule_weight(row, playthrough) if row[0x36] & 0x01 else 0
        packed.extend(
            struct.pack(
                "<HHfHHBBH",
                key,
                weight,
                struct.unpack_from("<f", row, 0x14)[0],
                identities[0],
                identities[1],
                active_mask,
                scratch_bit_by_key.get(key, 0xFF),
                0,
            )
        )
    return scratch_keys, enemy_groups, bytes(packed)


def generate_special_rule_match_masks_batch(
    displayed_seeds: Sequence[int],
    terrain_row_indices: Sequence[int],
    playthrough: int,
    *,
    criteria: AuxiliarySearchCriteria,
) -> tuple[int, ...]:
    """Match special-rule groups on CUDA after exact enemy scratch replay."""

    seeds = tuple(int(seed) & 0xFFFFFFFF for seed in displayed_seeds)
    terrain_rows = tuple(int(row) for row in terrain_row_indices)
    if len(seeds) != len(terrain_rows):
        raise ValueError("special-rule matcher requires one terrain row per Seed")
    groups = tuple(
        (frozenset((key,)) for key in sorted(criteria.required_special_rule_keys))
    ) + tuple(criteria.required_special_rule_key_groups)
    if not groups:
        return (0,) * len(seeds)
    if not seeds:
        return ()
    _scratch_keys, scratch_enemy_groups, packed_rule_rows = (
        _special_rule_batch_configuration(playthrough)
    )
    (
        mode_threshold,
        descriptor_thresholds,
        selector_threshold,
        role_five_threshold,
        selector_value,
        packed_enemy_rows,
        packed_terrains,
        packed_contexts,
    ) = _enemy_batch_configuration()
    scratch_masks = match_enemy_constraints_native(
        seeds,
        terrain_rows,
        playthrough=playthrough,
        mode_threshold=mode_threshold,
        descriptor_thresholds=descriptor_thresholds,
        selector_threshold=selector_threshold,
        role_five_threshold=role_five_threshold,
        selector_value=selector_value,
        enemy_rows=packed_enemy_rows,
        terrains=packed_terrains,
        contexts=packed_contexts,
        criterion_groups=scratch_enemy_groups,
    )
    if scratch_masks is None or last_seed_acceleration_backend() != "cuda":
        raise RuntimeError(
            "CUDA enemy scratch-key matcher is unavailable; CPU fallback is disabled"
        )
    native = match_special_rule_constraints_native(
        seeds,
        scratch_masks,
        rule_rows=packed_rule_rows,
        criterion_groups=groups,
    )
    if native is None or last_seed_acceleration_backend() != "cuda":
        raise RuntimeError(
            "CUDA special-rule matcher is unavailable; CPU fallback is disabled"
        )
    return native


def derive_enemy_seed(displayed_seed: int) -> int:
    """Reproduce the 28-bit scoped seed installed at RVA 0x10295B4..0x10295D7."""

    return displayed_seed & 0x0FFFFFFF


def _enemy_cost(row: bytes) -> float:
    return struct.unpack_from("<f", row, 0x0C)[0]


def _enemy_lookup_key(row: bytes) -> int:
    return struct.unpack_from("<I", row, 0x04)[0]


def _enemy_entry(row_index: int, row: bytes) -> EnemyEntryResult:
    return EnemyEntryResult(
        row_index=row_index,
        lookup_key=_enemy_lookup_key(row),
        role=row[0x1A],
        scratch_rule_key=struct.unpack_from("<H", row, 0x12)[0],
    )


def _draw_65535(rng: Lcg32) -> int:
    """Return the exact 16-bit ticket produced by the native RNG helper.

    RVA 0x1029B20 and its sibling sites multiply the normalized RNG result by
    the float constant at RVA 0x3BCCA7C.  That constant is 65536.0, not
    65535.0, so the conversion recovers the full high 16 bits exactly.
    """

    return rng.next_u16()


def _enemy_parameter_gate_accepts(
    row: bytes,
    *,
    descriptor_flag_22: bool,
    enemy_param_type_by_key: Mapping[int, int],
) -> bool:
    parameter_type = enemy_param_type_by_key.get(_enemy_lookup_key(row))
    if parameter_type is None:
        return True
    neutral_type = parameter_type in (0, 3)
    if descriptor_flag_22:
        return neutral_type
    if not neutral_type:
        return True
    return row[0x1A] in (4, 5)


def _enemy_terrain_gate_accepts(row: bytes, terrain_row: bytes) -> bool:
    mask = struct.unpack_from("<H", row, 0x14)[0]
    for bit in range(3):
        if not (mask & (1 << bit)):
            continue
        if bit == 0:
            blocked = struct.unpack_from("<H", terrain_row, 0x2C)[0] != 0
        else:
            blocked = bool(
                (struct.unpack_from("<H", terrain_row, 0x2E)[0] >> (bit - 1))
                & 1
            )
        if blocked:
            return False
    return True


def _select_enemy_by_u16_ticket(
    row_indices: Sequence[int],
    rows: Sequence[bytes],
    budget: float,
    ticket: int,
) -> int | None:
    eligible = [index for index in row_indices if _enemy_cost(rows[index]) <= budget]
    if not eligible:
        return None
    return eligible[ticket % len(eligible)]


def _run_enemy_budget_helper(
    candidate_indices: Sequence[int],
    rows: Sequence[bytes],
    budget: float,
    rng: Lcg32,
    *,
    first_group: bool,
    output: list[int],
) -> int:
    """Pointer-free parity for the count-hint-zero path of RVA 0x1026FD0."""

    local_state = _draw_65535(rng)
    draws = 1
    remaining = f32(budget)
    if remaining <= 0.0:
        return draws

    while True:
        eligible = [
            index
            for index in candidate_indices
            if _enemy_cost(rows[index]) <= remaining
        ]
        if not eligible:
            return draws
        if first_group:
            maximum_cost = max(_enemy_cost(rows[index]) for index in eligible)
            eligible = [
                index
                for index in eligible
                if _enemy_cost(rows[index]) == maximum_cost
            ]

        local_state = (local_state * 0x10DCD + 1) & 0xFFFFFFFF
        selected = eligible[(local_state >> 16) % len(eligible)]
        output.append(selected)
        if first_group:
            return draws
        remaining = f32_sub(remaining, _enemy_cost(rows[selected]))
        if remaining <= 0.0:
            return draws


def _run_class0_budget_selection(
    candidate_indices: Sequence[int],
    rows: Sequence[bytes],
    budget: float,
    rng: Lcg32,
    *,
    output: list[int],
) -> int:
    """Pointer-free parity for the inline selector at RVA 0x102AD10.

    The first accepted row is restricted to the maximum affordable cost.
    Subsequent rows, if any budget remains, use the complete affordable pool.
    All local selections share one state seeded by a single global RNG draw.
    """

    local_state = _draw_65535(rng)
    remaining = f32(budget)
    prefer_maximum_cost = True
    while remaining > 0.0:
        eligible = [
            index
            for index in candidate_indices
            if _enemy_cost(rows[index]) <= remaining
        ]
        if not eligible:
            break
        if prefer_maximum_cost:
            maximum_cost = max(_enemy_cost(rows[index]) for index in eligible)
            eligible = [
                index
                for index in eligible
                if _enemy_cost(rows[index]) == maximum_cost
            ]

        local_state = (local_state * 0x10DCD + 1) & 0xFFFFFFFF
        selected = eligible[(local_state >> 16) % len(eligible)]
        output.append(selected)
        remaining = f32_sub(remaining, _enemy_cost(rows[selected]))
        prefer_maximum_cost = False
    return 1


def generate_class0_enemies(
    displayed_seed: int,
    playthrough: int,
    auxiliary_mode: int,
    terrain_row_index: int,
    *,
    descriptor_selector: int = 0,
    descriptor_flags: Sequence[bool] = (False, False, False),
    caller_option: int = 0,
    tables: AuxiliaryGenerationTables,
    resource: R4FinalizerResourceBundle | None = None,
    enemy_param_type_by_key: Mapping[int, int] | None = None,
) -> Class0EnemyResult:
    """Generate ordered class-0 enemy keys from native RVA 0x102AB7A.

    The generator works backwards through the active context budgets. The
    highest group always uses role 5 when that pool is available. Lower groups
    use the optional-multiplier row keyed by 0xCEFC to choose role 4 or role 5.
    Accepted rows and rows sharing their nonzero group byte are removed from
    both pools before the next budget is processed. Native display order is the
    reverse of that generation order.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if not 0 <= auxiliary_mode <= 0xFF:
        raise ValueError("auxiliary_mode must fit in one byte")
    if not 0 <= descriptor_selector <= 0xFF:
        raise ValueError("descriptor_selector must fit in one byte")
    if len(descriptor_flags) != 3:
        raise ValueError("descriptor_flags must contain bytes +0x21..+0x23")
    if caller_option != 0:
        raise AuxiliaryGenerationError(
            "class-0 caller-option path is not yet certified for offline parity"
        )
    if tables.enemy_candidates is None or tables.special_context is None:
        raise AuxiliaryGenerationError(
            "enemy candidate and special-context tables are unavailable"
        )
    gate = (
        enemy_param_type_by_key
        if enemy_param_type_by_key is not None
        else tables.enemy_param_type_by_key
    )
    if not gate:
        raise AuxiliaryGenerationError(
            "complete native enemy-parameter gate mapping is unavailable"
        )
    if not 0 <= terrain_row_index < tables.terrain.row_count:
        raise IndexError(terrain_row_index)
    if resource is None:
        resource = load_default_r4_finalizer_resource()

    context_matches = [
        row for row in tables.special_context.rows() if row[0x28] == auxiliary_mode
    ]
    if len(context_matches) != 1:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} did not resolve uniquely"
        )
    context_row = context_matches[0]
    if context_row[0x29] != 0:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} is not class 0"
        )

    candidate_rows = list(tables.enemy_candidates.rows())
    terrain_row = tables.terrain.row(terrain_row_index)
    flag_21, flag_22, _flag_23 = (bool(value) for value in descriptor_flags)
    candidate_indices: list[int] = []
    for index, row in enumerate(candidate_rows):
        if not (row[0x16] & (1 << (playthrough - 1))):
            continue
        if (
            descriptor_selector
            and row[0x1A] not in (4, 5)
            and row[0x19] != descriptor_selector
        ):
            continue
        if flag_21 and row[0x1B] and row[0x1B] != terrain_row[0x31]:
            continue
        if not _enemy_parameter_gate_accepts(
            row,
            descriptor_flag_22=flag_22,
            enemy_param_type_by_key=gate,
        ):
            continue
        if not _enemy_terrain_gate_accepts(row, terrain_row):
            continue
        candidate_indices.append(index)

    role_four = [
        index for index in candidate_indices if candidate_rows[index][0x1A] == 4
    ]
    role_five = [
        index for index in candidate_indices if candidate_rows[index][0x1A] == 5
    ]
    budgets = [
        struct.unpack_from("<f", context_row, 4 + index * 4)[0]
        for index in range(5)
    ]
    active_count = next(
        (index for index, value in enumerate(budgets) if value <= 0.0),
        len(budgets),
    )
    if active_count == 0:
        return Class0EnemyResult((), derive_enemy_seed(displayed_seed), 0)

    role_five_threshold = _optional_multiplier_threshold(
        resource.table("optional_multiplier"), 0xCEFC
    )
    rng = Lcg32(derive_enemy_seed(displayed_seed))
    draws = 0
    generated_groups: list[EnemyGroupResult] = []
    highest_index = active_count - 1
    for group_index in range(highest_index, -1, -1):
        use_role_five = bool(role_five)
        if role_five and group_index != highest_index:
            use_role_five = _roll_10000(rng) >= role_five_threshold
            draws += 1
        pool = role_five if use_role_five else role_four
        selected: list[int] = []
        draws += _run_class0_budget_selection(
            pool,
            candidate_rows,
            f32(budgets[group_index]),
            rng,
            output=selected,
        )
        generated_groups.append(
            EnemyGroupResult(
                entries=tuple(
                    _enemy_entry(index, candidate_rows[index]) for index in selected
                ),
                source_budget=f32(budgets[group_index]),
            )
        )

        for selected_index in selected:
            if selected_index in pool:
                pool.remove(selected_index)
            linked_group = candidate_rows[selected_index][0x18]
            if linked_group:
                role_five[:] = [
                    index
                    for index in role_five
                    if candidate_rows[index][0x18] != linked_group
                ]
                role_four[:] = [
                    index
                    for index in role_four
                    if candidate_rows[index][0x18] != linked_group
                ]

    generated_groups.reverse()
    return Class0EnemyResult(
        groups=tuple(generated_groups),
        scoped_seed=derive_enemy_seed(displayed_seed),
        random_draws=draws,
    )


def generate_class1_enemies(
    displayed_seed: int,
    playthrough: int,
    auxiliary_mode: int,
    terrain_row_index: int,
    *,
    descriptor_selector: int = 0,
    descriptor_flags: Sequence[bool] = (False, False, False),
    caller_option: int = 0,
    tables: AuxiliaryGenerationTables,
    enemy_param_type_by_key: Mapping[int, int] | None = None,
) -> Class1EnemyResult:
    """Generate ordered class-1 enemy keys from native RVA 0x102A259.

    This implementation deliberately requires the independent enemy-parameter
    gate captured from native table rows. Missing hash keys match native lookup
    failure and remain eligible. The caller-option path invokes a separate
    Mersenne-Twister helper and is not accepted until it has its own vectors.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if not 0 <= auxiliary_mode <= 0xFF:
        raise ValueError("auxiliary_mode must fit in one byte")
    if not 0 <= descriptor_selector <= 0xFF:
        raise ValueError("descriptor_selector must fit in one byte")
    if len(descriptor_flags) != 3:
        raise ValueError("descriptor_flags must contain bytes +0x21..+0x23")
    if caller_option != 0:
        raise AuxiliaryGenerationError(
            "class-1 caller-option path is not yet certified for offline parity"
        )
    if tables.enemy_candidates is None or tables.special_context is None:
        raise AuxiliaryGenerationError(
            "enemy candidate and special-context tables are unavailable"
        )
    gate = (
        enemy_param_type_by_key
        if enemy_param_type_by_key is not None
        else tables.enemy_param_type_by_key
    )
    if not gate:
        raise AuxiliaryGenerationError(
            "complete native enemy-parameter gate mapping is unavailable"
        )
    if not 0 <= terrain_row_index < tables.terrain.row_count:
        raise IndexError(terrain_row_index)

    context_matches = [
        row for row in tables.special_context.rows() if row[0x28] == auxiliary_mode
    ]
    if len(context_matches) != 1:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} did not resolve uniquely"
        )
    context_row = context_matches[0]
    if context_row[0x29] != 1:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} is not class 1"
        )

    candidate_rows = list(tables.enemy_candidates.rows())
    terrain_row = tables.terrain.row(terrain_row_index)
    flag_21, flag_22, flag_23 = (bool(value) for value in descriptor_flags)
    candidate_indices: list[int] = []
    for index, row in enumerate(candidate_rows):
        if not (row[0x16] & (1 << (playthrough - 1))):
            continue
        if (
            descriptor_selector
            and row[0x1A] not in (4, 5)
            and row[0x19] != descriptor_selector
        ):
            continue
        if flag_21 and row[0x1B] and row[0x1B] != terrain_row[0x31]:
            continue
        if not _enemy_parameter_gate_accepts(
            row,
            descriptor_flag_22=flag_22,
            enemy_param_type_by_key=gate,
        ):
            continue
        if not _enemy_terrain_gate_accepts(row, terrain_row):
            continue
        candidate_indices.append(index)

    general = [
        index for index in candidate_indices if candidate_rows[index][0x1A] not in (4, 5)
    ]
    role_five = [
        index for index in candidate_indices if candidate_rows[index][0x1A] == 5
    ]
    role_zero_or_two = [
        index for index in general if candidate_rows[index][0x1A] in (0, 2)
    ]
    role_not_zero_or_two = [
        index for index in general if candidate_rows[index][0x1A] not in (0, 2)
    ]

    budgets = [struct.unpack_from("<f", context_row, 4 + index * 4)[0] for index in range(5)]
    active_count = next(
        (index for index, value in enumerate(budgets) if value <= 0.0),
        len(budgets),
    )
    if active_count == 0:
        return Class1EnemyResult((), derive_enemy_seed(displayed_seed), 0)

    rng = Lcg32(derive_enemy_seed(displayed_seed))
    draws = 0
    generated_groups: list[EnemyGroupResult] = []
    highest_index = active_count - 1
    for group_index in range(highest_index, -1, -1):
        source_budget = f32(budgets[group_index])
        remaining = source_budget
        work_pool = list(general)
        selected: list[int] = []

        if group_index != highest_index and not flag_23:
            first = _select_enemy_by_u16_ticket(
                role_not_zero_or_two,
                candidate_rows,
                remaining,
                _draw_65535(rng),
            )
            draws += 1
            if first is not None:
                selected.append(first)
                remaining = f32_sub(remaining, _enemy_cost(candidate_rows[first]))

            second = _select_enemy_by_u16_ticket(
                role_zero_or_two,
                candidate_rows,
                remaining,
                _draw_65535(rng),
            )
            draws += 1
            if second is not None:
                second_group = candidate_rows[second][0x18]
                replacements = [second]
                if second_group:
                    replacements.extend(
                        index
                        for index in general
                        if index != second
                        and candidate_rows[index][0x18] == second_group
                    )
                replacement_index = 0
                for position, index in enumerate(work_pool):
                    if candidate_rows[index][0x1A] in (0, 2):
                        work_pool[position] = replacements[
                            replacement_index % len(replacements)
                        ]
                        replacement_index += 1

        draws += _run_enemy_budget_helper(
            role_five if group_index == highest_index else work_pool,
            candidate_rows,
            remaining,
            rng,
            first_group=group_index == highest_index,
            output=selected,
        )
        generated_groups.append(
            EnemyGroupResult(
                entries=tuple(
                    _enemy_entry(index, candidate_rows[index]) for index in selected
                ),
                source_budget=source_budget,
            )
        )

    generated_groups.reverse()
    return Class1EnemyResult(
        groups=tuple(generated_groups),
        scoped_seed=derive_enemy_seed(displayed_seed),
        random_draws=draws,
    )


def generate_class2_enemies(
    displayed_seed: int,
    playthrough: int,
    auxiliary_mode: int,
    terrain_row_index: int,
    *,
    descriptor_selector: int = 0,
    descriptor_flags: Sequence[bool] = (False, False, False),
    caller_option: int = 0,
    tables: AuxiliaryGenerationTables,
    enemy_param_type_by_key: Mapping[int, int] | None = None,
) -> Class2EnemyResult:
    """Generate ordered class-2 enemy keys from native RVA 0x1029A70.

    The normal UI caller passes ``caller_option == 0``. For each active budget
    row, this class consumes two global tickets for pool shaping and one ticket
    to seed the local budget helper. The second preselection changes the
    role-0/role-2 candidates but does not consume budget; this otherwise
    surprising behavior is certified against twelve native vectors spanning
    modes 0x48, 0x62, and 0x8E.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if not 0 <= auxiliary_mode <= 0xFF:
        raise ValueError("auxiliary_mode must fit in one byte")
    if not 0 <= descriptor_selector <= 0xFF:
        raise ValueError("descriptor_selector must fit in one byte")
    if len(descriptor_flags) != 3:
        raise ValueError("descriptor_flags must contain bytes +0x21..+0x23")
    if caller_option != 0:
        raise AuxiliaryGenerationError(
            "class-2 caller-option path is not yet certified for offline parity"
        )
    if tables.enemy_candidates is None or tables.special_context is None:
        raise AuxiliaryGenerationError(
            "enemy candidate and special-context tables are unavailable"
        )
    gate = (
        enemy_param_type_by_key
        if enemy_param_type_by_key is not None
        else tables.enemy_param_type_by_key
    )
    if not gate:
        raise AuxiliaryGenerationError(
            "complete native enemy-parameter gate mapping is unavailable"
        )
    if not 0 <= terrain_row_index < tables.terrain.row_count:
        raise IndexError(terrain_row_index)

    context_matches = [
        row for row in tables.special_context.rows() if row[0x28] == auxiliary_mode
    ]
    if len(context_matches) != 1:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} did not resolve uniquely"
        )
    context_row = context_matches[0]
    if context_row[0x29] != 2:
        raise AuxiliaryGenerationError(
            f"auxiliary mode 0x{auxiliary_mode:02X} is not class 2"
        )

    candidate_rows = list(tables.enemy_candidates.rows())
    terrain_row = tables.terrain.row(terrain_row_index)
    flag_21, flag_22, flag_23 = (bool(value) for value in descriptor_flags)
    candidate_indices: list[int] = []
    for index, row in enumerate(candidate_rows):
        if not (row[0x16] & (1 << (playthrough - 1))):
            continue
        if (
            descriptor_selector
            and row[0x1A] not in (4, 5)
            and row[0x19] != descriptor_selector
        ):
            continue
        if flag_21 and row[0x1B] and row[0x1B] != terrain_row[0x31]:
            continue
        if not _enemy_parameter_gate_accepts(
            row,
            descriptor_flag_22=flag_22,
            enemy_param_type_by_key=gate,
        ):
            continue
        if not _enemy_terrain_gate_accepts(row, terrain_row):
            continue
        candidate_indices.append(index)

    general = [
        index for index in candidate_indices if candidate_rows[index][0x1A] not in (4, 5)
    ]
    role_zero_or_two = [
        index for index in general if candidate_rows[index][0x1A] in (0, 2)
    ]
    role_not_zero_or_two = [
        index for index in general if candidate_rows[index][0x1A] not in (0, 2)
    ]

    budgets = [
        struct.unpack_from("<f", context_row, 4 + index * 4)[0]
        for index in range(5)
    ]
    active_count = next(
        (index for index, value in enumerate(budgets) if value <= 0.0),
        len(budgets),
    )
    if active_count == 0:
        return Class2EnemyResult((), derive_enemy_seed(displayed_seed), 0)

    rng = Lcg32(derive_enemy_seed(displayed_seed))
    draws = 0
    generated_groups: list[EnemyGroupResult] = []
    for group_index in range(active_count):
        source_budget = f32(budgets[group_index])
        remaining = source_budget
        work_pool = list(general)
        selected: list[int] = []

        if not flag_23:
            first = _select_enemy_by_u16_ticket(
                role_not_zero_or_two,
                candidate_rows,
                remaining,
                _draw_65535(rng),
            )
            draws += 1
            if first is not None:
                selected.append(first)
                remaining = f32_sub(remaining, _enemy_cost(candidate_rows[first]))

            second = _select_enemy_by_u16_ticket(
                role_zero_or_two,
                candidate_rows,
                remaining,
                _draw_65535(rng),
            )
            draws += 1
            if second is not None:
                second_group = candidate_rows[second][0x18]
                replacements = [second]
                if second_group:
                    replacements.extend(
                        index
                        for index in general
                        if index != second
                        and candidate_rows[index][0x18] == second_group
                    )
                replacement_index = 0
                for position, index in enumerate(work_pool):
                    if candidate_rows[index][0x1A] in (0, 2):
                        work_pool[position] = replacements[
                            replacement_index % len(replacements)
                        ]
                        replacement_index += 1

        draws += _run_enemy_budget_helper(
            work_pool,
            candidate_rows,
            remaining,
            rng,
            first_group=False,
            output=selected,
        )
        generated_groups.append(
            EnemyGroupResult(
                entries=tuple(
                    _enemy_entry(index, candidate_rows[index]) for index in selected
                ),
                source_budget=source_budget,
            )
        )

    return Class2EnemyResult(
        groups=tuple(generated_groups),
        scoped_seed=derive_enemy_seed(displayed_seed),
        random_draws=draws,
    )


def _rule_weight(row: bytes, playthrough: int) -> int:
    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    return struct.unpack_from("<H", row, 0x20 + playthrough * 2)[0]


def legal_special_rule_keys(
    playthrough: int,
    *,
    tables: AuxiliaryGenerationTables | None = None,
) -> frozenset[int]:
    """Return rule keys that the native generator can actually select.

    The captured table includes disabled placeholders and rows whose weight is
    zero for a given playthrough.  Those rows remain useful research evidence,
    but exposing them in the product picker would promise an impossible result.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    if tables.special_rules is None:
        raise AuxiliaryGenerationError("special-rule table is unavailable")
    if len(tables.special_rule_keys_by_row) != tables.special_rules.row_count:
        raise AuxiliaryGenerationError("special-rule key index does not match row count")
    return frozenset(
        key
        for key, row in zip(
            tables.special_rule_keys_by_row,
            tables.special_rules.rows(),
            strict=True,
        )
        if (row[0x36] & 0x01) and _rule_weight(row, playthrough) > 0
    )


def _rule_rows_conflict(
    current: bytes,
    previous: bytes,
    *,
    conflict_rows_by_key: dict[int, bytes],
) -> bool:
    # RVA 0x1026680 resolves both +0x2C/+0x2E group keys through manager
    # +0x20. Only group rows whose +0x0C bit 0 is set participate.
    current_groups = struct.unpack_from("<HH", current, 0x2C)
    previous_groups = struct.unpack_from("<HH", previous, 0x2C)
    for current_key in current_groups:
        current_group = conflict_rows_by_key.get(current_key)
        if current_group is None or not (current_group[0x0C] & 0x01):
            continue
        current_identity = struct.unpack_from("<H", current_group, 0x08)[0]
        for previous_key in previous_groups:
            previous_group = conflict_rows_by_key.get(previous_key)
            if previous_group is None:
                continue
            previous_identity = struct.unpack_from("<H", previous_group, 0x08)[0]
            if current_identity == previous_identity:
                return True
    return False


def generate_special_rules(
    displayed_seed: int,
    playthrough: int,
    scratch_rule_keys: Sequence[int] = (0xFFFF, 0xFFFF, 0xFFFF),
    *,
    tables: AuxiliaryGenerationTables,
) -> SpecialRuleResult:
    """Generate the three ordered rule keys produced by native RVA 0x1028880."""

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if any(not 0 <= key <= 0xFFFF for key in scratch_rule_keys):
        raise ValueError("scratch rule keys must fit in uint16")
    if tables.special_rules is None or tables.rule_conflicts is None:
        raise AuxiliaryGenerationError("special-rule tables are unavailable")
    if len(tables.special_rule_keys_by_row) != tables.special_rules.row_count:
        raise AuxiliaryGenerationError("special-rule key index does not match row count")
    if len(tables.rule_conflict_keys_by_row) != tables.rule_conflicts.row_count:
        raise AuxiliaryGenerationError("rule-conflict key index does not match row count")

    rule_rows = list(tables.special_rules.rows())
    conflict_rows_by_key = dict(
        zip(
            tables.rule_conflict_keys_by_row,
            tables.rule_conflicts.rows(),
            strict=True,
        )
    )
    blocked = set(scratch_rule_keys)

    scoped_seed = derive_special_rule_seed(displayed_seed)
    rng = Lcg32(scoped_seed)
    target_budget = rng.random_int(5) + 1
    draws = 1
    remaining = f32(target_budget)
    original_budget = f32(target_budget)
    selected_keys: list[int] = []
    selected_rows: list[bytes] = []
    zero_selected = False
    third_slot_best_abs = f32(0.0)

    for _attempt in range(3):
        candidates: list[tuple[int, bytes, int]] = []
        total_weight = 0
        accepted_count = len(selected_keys)

        for row in rule_rows:
            if not (row[0x36] & 0x01):
                continue
            key = struct.unpack_from("<H", row, 0x20)[0]
            if key in blocked:
                continue

            if accepted_count:
                if key == 0:
                    if zero_selected:
                        continue
                else:
                    if key in selected_keys:
                        continue
                    if any(
                        _rule_rows_conflict(
                            row,
                            previous,
                            conflict_rows_by_key=conflict_rows_by_key,
                        )
                        for previous in selected_rows
                    ):
                        continue

            cost = struct.unpack_from("<f", row, 0x14)[0]
            if accepted_count == 1:
                accumulated_delta = f32_sub(remaining, original_budget)
                if accumulated_delta < 0.0 and cost >= 0.0:
                    continue
                if accumulated_delta > 0.0 and cost <= 0.0:
                    continue
            elif accepted_count == 2:
                if remaining < 0.0 and cost > 0.0:
                    continue
                if remaining > 0.0 and cost < 0.0:
                    continue
                absolute_cost = f32(abs(cost))
                if absolute_cost > f32(abs(remaining)):
                    continue
                if absolute_cost < third_slot_best_abs:
                    continue
                if absolute_cost > third_slot_best_abs:
                    candidates.clear()
                    total_weight = 0
                    third_slot_best_abs = absolute_cost

            weight = _rule_weight(row, playthrough)
            candidates.append((key, row, weight))
            total_weight = (total_weight + weight) & 0xFFFF

        if total_weight == 0:
            break

        ticket = rng.random_int(total_weight)
        draws += 1
        chosen: tuple[int, bytes, int] | None = None
        for candidate in candidates:
            weight = candidate[2]
            if ticket < weight:
                chosen = candidate
                break
            ticket -= weight
        if chosen is None:
            raise AuxiliaryGenerationError("special-rule weighted lottery had no winner")

        key, row, _weight = chosen
        selected_keys.append(key)
        selected_rows.append(row)
        if key == 0:
            zero_selected = True
        remaining = f32_sub(remaining, struct.unpack_from("<f", row, 0x14)[0])
        if remaining == 0.0:
            break

    compacted = [key for key in selected_keys if key != 0]
    keys = tuple((compacted + [0, 0, 0])[:3])
    entries = tuple(
        describe_special_rule(key, tables=tables)
        for key in keys
        if key != 0
    )
    return SpecialRuleResult(
        keys=(int(keys[0]), int(keys[1]), int(keys[2])),
        entries=entries,
        scoped_seed=scoped_seed,
        target_budget=target_budget,
        random_draws=draws,
    )


def describe_special_rule(
    key: int,
    *,
    tables: AuxiliaryGenerationTables | None = None,
) -> SpecialRuleEntryResult:
    """Decode the scroll-detail UI value path for one native rule key.

    The consumer at RVA 0x1F0D625 selects a formatting family from row fields.
    Percentage values are divided by the native 10.0 constant. Duration values
    are divided by 60.0 and formatted as seconds. The three rows tagged 0x2FC9
    are ordered and displayed as C, B, and A instead of a number.
    """

    if not 0 <= key <= 0xFFFF:
        raise ValueError("special-rule key must fit in uint16")
    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    if tables.special_rules is None:
        raise AuxiliaryGenerationError("special-rule table is unavailable")
    matches = [
        (row_index, row)
        for row_index, (row_key, row) in enumerate(
            zip(
                tables.special_rule_keys_by_row,
                tables.special_rules.rows(),
                strict=True,
            )
        )
        if row_key == key
    ]
    if len(matches) != 1:
        raise AuxiliaryGenerationError(
            f"special-rule key 0x{key:04X} resolved to {len(matches)} rows"
        )
    row_index, row = matches[0]

    qualifier_kind: str | None = None
    qualifier_key: int | None = None
    qualifier_text_id = struct.unpack_from("<I", row, 0x08)[0]
    enemy_key = struct.unpack_from("<I", row, 0x04)[0]
    item_key = struct.unpack_from("<H", row, 0x30)[0]
    effect_key = struct.unpack_from("<H", row, 0x32)[0]
    if qualifier_text_id:
        qualifier_kind, qualifier_key = "text", qualifier_text_id
    elif item_key:
        qualifier_kind, qualifier_key = "item", item_key
    elif enemy_key:
        qualifier_kind, qualifier_key = "enemy", enemy_key
    elif effect_key:
        qualifier_kind, qualifier_key = "effect", effect_key

    group_key = struct.unpack_from("<H", row, 0x2C)[0]
    if group_key == 0x2FC9:
        ordered_values = sorted(
            struct.unpack_from("<f", candidate, 0x18)[0]
            for candidate in tables.special_rules.rows()
            if struct.unpack_from("<H", candidate, 0x2C)[0] == group_key
        )
        raw_value = struct.unpack_from("<f", row, 0x18)[0]
        rank = ordered_values.index(raw_value)
        grade = "B" if rank == 1 else ("A" if rank == 2 else "C")
        return SpecialRuleEntryResult(
            key=key,
            row_index=row_index,
            raw_value=raw_value,
            display_value=None,
            display_unit="grade",
            display_grade=grade,
            value_source_offset=0x18,
            qualifier_kind=qualifier_kind,
            qualifier_key=qualifier_key,
        )

    if qualifier_text_id:
        source_offset = 0x1C
        raw_value = struct.unpack_from("<f", row, source_offset)[0]
        display_value = raw_value / 10.0
        display_unit = "percent"
    elif item_key:
        source_offset = 0x18
        raw_value = struct.unpack_from("<f", row, source_offset)[0]
        display_value = raw_value / 60.0
        display_unit = "seconds"
    elif enemy_key:
        source_offset = None
        raw_value = None
        display_value = None
        display_unit = None
    else:
        source_offset = 0x18
        raw_value = struct.unpack_from("<f", row, source_offset)[0]
        if raw_value == 0.0:
            source_offset = None
            raw_value = None
            display_value = None
            display_unit = None
        else:
            display_value = raw_value / 10.0
            display_unit = "percent"

    return SpecialRuleEntryResult(
        key=key,
        row_index=row_index,
        raw_value=raw_value,
        display_value=display_value,
        display_unit=display_unit,
        display_grade=None,
        value_source_offset=source_offset,
        qualifier_kind=qualifier_kind,
        qualifier_key=qualifier_key,
    )


def generate_matching_auxiliary(
    displayed_seed: int,
    playthrough: int,
    *,
    criteria: AuxiliarySearchCriteria,
    caller_option: int = 0,
    tables: AuxiliaryGenerationTables | None = None,
    resource: R4FinalizerResourceBundle | None = None,
    stage_acceptor: Callable[[str, object], bool] | None = None,
) -> CompleteAuxiliaryResult | None:
    """Generate auxiliary output with exact component-level early rejection.

    Enemy-provided scratch rule keys are fed into the rule generator exactly as
    the native descriptor builder does. This matters even when the scratch key
    is not itself displayed: it removes that rule from the later weighted pool.
    """

    if not 1 <= playthrough <= 5:
        raise ValueError("playthrough must be in 1..5")
    if tables is None:
        tables = load_default_auxiliary_generation_tables()
    if resource is None:
        resource = load_default_r4_finalizer_resource()

    mode = generate_auxiliary_mode(displayed_seed, resource=resource)
    terrain = generate_terrain(
        displayed_seed,
        mode.value,
        tables=tables,
        resource=resource,
    )
    if stage_acceptor is not None:
        if not stage_acceptor("terrain", terrain):
            return None
    elif not criteria.matches_terrain(terrain):
        return None
    descriptor = generate_auxiliary_descriptor_flags(
        displayed_seed,
        mode.value,
        resource=resource,
        tables=tables,
    )
    common = {
        "descriptor_selector": descriptor.selector,
        "descriptor_flags": descriptor.flags,
        "caller_option": caller_option,
        "tables": tables,
    }
    if mode.branch_class == 0:
        enemies: EnemyGenerationResult = generate_class0_enemies(
            displayed_seed,
            playthrough,
            mode.value,
            terrain.selected_row_index,
            resource=resource,
            **common,
        )
    elif mode.branch_class == 1:
        enemies = generate_class1_enemies(
            displayed_seed,
            playthrough,
            mode.value,
            terrain.selected_row_index,
            **common,
        )
    elif mode.branch_class == 2:
        enemies = generate_class2_enemies(
            displayed_seed,
            playthrough,
            mode.value,
            terrain.selected_row_index,
            **common,
        )
    else:
        raise AuxiliaryGenerationError(
            f"unsupported auxiliary branch class {mode.branch_class}"
        )
    if stage_acceptor is not None:
        if not stage_acceptor("enemy", enemies):
            return None
    elif not criteria.matches_enemies(enemies):
        return None

    scratch_rule_keys = tuple(
        entry.scratch_rule_key
        for group in enemies.groups
        for entry in group.entries
        if entry.scratch_rule_key != 0xFFFF
    )
    special_rules = generate_special_rules(
        displayed_seed,
        playthrough,
        scratch_rule_keys,
        tables=tables,
    )
    if stage_acceptor is not None:
        if not stage_acceptor("rule", special_rules):
            return None
    elif not criteria.matches_special_rules(special_rules):
        return None
    return CompleteAuxiliaryResult(
        mode=mode,
        terrain=terrain,
        descriptor=descriptor,
        enemies=enemies,
        special_rules=special_rules,
    )


def generate_complete_auxiliary(
    displayed_seed: int,
    playthrough: int,
    *,
    caller_option: int = 0,
    tables: AuxiliaryGenerationTables | None = None,
    resource: R4FinalizerResourceBundle | None = None,
) -> CompleteAuxiliaryResult:
    """Generate complete deterministic auxiliary output without filtering."""

    result = generate_matching_auxiliary(
        displayed_seed,
        playthrough,
        criteria=AuxiliarySearchCriteria(),
        caller_option=caller_option,
        tables=tables,
        resource=resource,
    )
    if result is None:  # Empty criteria must accept every valid native result.
        raise AssertionError("empty auxiliary criteria rejected a generated result")
    return result


__all__ = [
    "AUXILIARY_DESCRIPTOR_SELECTOR_KEY",
    "AUXILIARY_DESCRIPTOR_THRESHOLD_KEYS",
    "AUXILIARY_MODE_SEED_MASK_HIGH",
    "AUXILIARY_MODE_SEED_MASK_LOW",
    "AUXILIARY_MODE_THRESHOLD_KEY",
    "AUXILIARY_RESOURCE_SCHEMA",
    "AuxiliaryDescriptorFlagsResult",
    "AuxiliaryGenerationError",
    "AuxiliaryGenerationTables",
    "AuxiliaryModeResult",
    "AuxiliarySearchCriteria",
    "Class0EnemyResult",
    "Class1EnemyResult",
    "Class2EnemyResult",
    "CompleteAuxiliaryResult",
    "DEFAULT_AUXILIARY_RESOURCE_ROOT",
    "ENEMY_PARAMETER_GATE_CAPTURE_SCHEMA",
    "EnemyEntryResult",
    "EnemyGroupResult",
    "EnemyGenerationResult",
    "SpecialRuleResult",
    "SpecialRuleEntryResult",
    "TERRAIN_DISPLAY_CRUCIBLE_KEY",
    "TERRAIN_DISPLAY_SPECIAL_KEYS",
    "TerrainResult",
    "build_auxiliary_generation_resource",
    "derive_auxiliary_descriptor_seed",
    "derive_auxiliary_mode_seed",
    "derive_enemy_seed",
    "derive_special_rule_seed",
    "derive_terrain_seed",
    "describe_special_rule",
    "generate_auxiliary_mode",
    "generate_auxiliary_descriptor_flags",
    "generate_class0_enemies",
    "generate_class1_enemies",
    "generate_class2_enemies",
    "generate_complete_auxiliary",
    "generate_enemy_match_masks_batch",
    "generate_matching_auxiliary",
    "generate_special_rules",
    "generate_special_rule_match_masks_batch",
    "generate_terrain",
    "generate_terrain_row_indices_batch",
    "legal_special_rule_keys",
    "load_default_auxiliary_generation_tables",
    "load_enemy_parameter_gate_capture",
    "terrain_display_effect_keys_for_row",
    "terrain_row_matches_criteria",
    "terrain_rows_containing_effects",
]
