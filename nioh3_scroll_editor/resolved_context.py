"""Version-bound resolved generation identity for the Python worker.

This module mirrors the accepted Rust contract in
``crates/nioh3-worker/src/context.rs`` plus its selected-bundle resolver in
``crates/nioh3-data/src/selected_bundle.rs``. Python and Rust must publish the
same production identity, so the canonical encoder below is byte-for-byte
compatible with the Rust one:

* both digests are SHA-256 over a UTF-8 JSON object with sorted keys and the
  compact ``,``/``:`` separators, i.e. the payload is the value of
  ``json.dumps(payload, sort_keys=True, separators=(",", ":"))``;
* ``game_file_version`` is the dotted four-part string, never a list or object;
* the digest is never part of its own hashed payload.

Two identities exist, and only one may authorize work:

* :class:`ResolvedGenerationContext` is the production identity. It folds the
  exact installed game ``file_version`` and the version-bound selected-bundle
  digests, so two data roots that only differ by which bundle they resolve no
  longer share an identity.
* The pre-version digest survives only as an explicit proof/diagnostic field
  (:attr:`ResolvedGenerationContext.legacy_context_digest`) and an opt-in,
  visibly non-production capture. It never authorizes a candidate, cache, or
  resume.

There is no ``CURRENT``/default version fallback on the production path: an
unknown or missing exact version fails closed before the data root is opened.
"""

from __future__ import annotations

import hashlib
import json
import struct
from dataclasses import dataclass
from functools import lru_cache
from pathlib import Path
from typing import Any

from .core_services import (
    GENERATION_ALGORITHM_VERSION,
    OPERATION_POLICY_VERSION,
    SUPPORTED_GAME_PROFILE,
    CoreErrorCode,
    CoreServiceError,
    runtime_resource_digest,
)
from .seed_accelerator import seed_accelerator_identity
from .version import APP_VERSION


PRODUCT_VERSION = APP_VERSION

RESOLVED_CONTEXT_SCHEMA = "nioh3-resolved-generation-context/v1"
LEGACY_CONTEXT_SCHEMA = "nioh3-legacy-generation-context/v1"

# Version-selected offline resource directory per exact executable version,
# mirroring ``r4_resource_dir_for_file_version``. PC v2.00.02 and PC v2.01
# alias the shipped directory because their deterministic payloads are
# byte-equal; PC v2.02 changed two tables and owns its own directory. A version
# that is absent here is unregistered and must fail closed.
VERSIONED_RESOURCE_DIRS: dict[tuple[int, int, int, int], str] = {
    (2, 0, 0, 2): "r4_finalizer/pc_v2_00_02/resource_v1",
    (2, 0, 1, 0): "r4_finalizer/pc_v2_00_02/resource_v1",
    (2, 0, 2, 0): "r4_finalizer/pc_v2_02/resource_v1",
}

# Version-invariant companions the versioned materialization also reads, so they
# belong to the bundle identity even though they are not versioned payloads.
AUXILIARY_RESOURCE_DIR = "auxiliary_generation/pc_v2_00_02/resource_v3"
ENEMY_STATE_TABLES_PATH = "enemy_states/pc_v2_01/native_tables.json"
GRACE_MAP_PATHS = (
    "grace_output_map_e604_r4_current.json",
    "grace_output_map_e604_r5_current.json",
)
R4_SCHEMA = "nioh3-r4-finalizer-resource/v1"
AUXILIARY_SCHEMA = "nioh3-auxiliary-generation-resource/v3"

# The nine R4 tables the versioned effect/context materialization reads, in the
# order ``resource_descriptor::R4_TABLES`` declares them.
R4_TABLE_NAMES = (
    "item",
    "effect_group",
    "category",
    "category_count_multiplier",
    "level_curve",
    "effect",
    "optional_multiplier",
    "rarity_roll",
    "special_context",
)
# Manifest-declared companion blobs below the versioned resource directory.
R4_COMPANION_RECORDS = (
    ("bonus_curve", "rows_file", "bonus_curve rows"),
    ("bonus_curve", "index_file", "bonus_curve index"),
    ("playthrough", "file", "playthrough"),
)
# The five auxiliary tables the roster and rule loaders read, and whether their
# declared ``keys_file`` index is a loader input.
AUXILIARY_TABLES = (
    ("auxiliary_terrain", True),
    ("auxiliary_enemy_candidate", False),
    ("special_context", False),
    ("scroll_special_rule", True),
    ("auxiliary_rule_conflict", True),
)
AUXILIARY_GATE_SECTION = "enemy_parameter_gate"
AUXILIARY_GATE_KEY = "file"


def normalize_file_version(file_version: Any) -> tuple[int, int, int, int]:
    """Coerce an explicit four-part executable version, rejecting anything else.

    Accepts a four-item sequence of integers or a dotted four-part string.
    Anything shorter, longer, non-integral, or out of range is refused so a
    malformed version cannot silently resolve.
    """

    if isinstance(file_version, (str, bytes, bytearray)):
        text = file_version.decode("utf-8") if isinstance(file_version, bytes) else file_version
        parts = text.strip().split(".")
    else:
        try:
            parts = list(file_version)
        except TypeError as error:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"explicit game file version is required, got {type(file_version).__name__}",
            ) from error
    if len(parts) != 4:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            "explicit game file version must have exactly four parts",
        )
    normalized: list[int] = []
    for part in parts:
        try:
            value = int(part)
        except (TypeError, ValueError) as error:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"game file version part {part!r} is not an integer",
            ) from error
        if not 0 <= value <= 0xFFFF:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"game file version part {value} is out of range",
            )
        normalized.append(value)
    return (normalized[0], normalized[1], normalized[2], normalized[3])


def dotted_file_version(file_version: Any) -> str:
    """Dotted four-part spelling, matching ``GameFileVersion::dotted``."""

    major, minor, patch, build = normalize_file_version(file_version)
    return f"{major}.{minor}.{patch}.{build}"


def _require_explicit_file_version(file_version: Any) -> tuple[int, int, int, int]:
    if file_version is None:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            "explicit game file version is required; there is no default version",
        )
    return normalize_file_version(file_version)


def _resolved_canonical_payload(
    *,
    product_version: str,
    game_profile: str,
    game_file_version: Any,
    versioned_resource_dir: str,
    bundle_digest: str,
    versioned_digest: str,
    resources_digest: str,
    algorithm_version: str,
    policy_version: str,
    seed_accelerator_abi: int | None,
    seed_accelerator_build_id: str | None,
) -> str:
    """Canonical resolved-identity JSON, byte-identical to the Rust encoder."""

    payload = {
        "algorithm_version": algorithm_version,
        "bundle_digest": bundle_digest,
        "game_file_version": dotted_file_version(game_file_version),
        "game_profile": game_profile,
        "policy_version": policy_version,
        "product_version": product_version,
        "resources_digest": resources_digest,
        "seed_accelerator_abi": seed_accelerator_abi,
        "seed_accelerator_build_id": seed_accelerator_build_id,
        "versioned_digest": versioned_digest,
        "versioned_resource_dir": versioned_resource_dir,
    }
    return json.dumps(payload, sort_keys=True, separators=(",", ":"))


def _legacy_canonical_payload(
    *,
    product_version: str,
    game_profile: str,
    resources_digest: str,
    algorithm_version: str,
    policy_version: str,
    seed_accelerator_abi: int | None,
    seed_accelerator_build_id: str | None,
) -> str:
    """Canonical pre-version identity JSON, matching the shipped seven keys."""

    payload = {
        "algorithm_version": algorithm_version,
        "game_profile": game_profile,
        "policy_version": policy_version,
        "product_version": product_version,
        "resources_digest": resources_digest,
        "seed_accelerator_abi": seed_accelerator_abi,
        "seed_accelerator_build_id": seed_accelerator_build_id,
    }
    return json.dumps(payload, sort_keys=True, separators=(",", ":"))


@dataclass(frozen=True, slots=True)
class SelectedGenerationBundle:
    """Identity of the generation resource bundle selected for one version.

    ``bundle_digest`` covers every selected input; ``versioned_digest`` covers
    only the files below the version-selected resource directory. Both fold
    ``len(u32 LE) || data-root-relative POSIX path || file bytes SHA-256`` in
    the same canonical order the whole-root digest uses.
    """

    file_version: tuple[int, int, int, int]
    versioned_resource_dir: str
    bundle_digest: str
    versioned_digest: str
    files: tuple[str, ...]


def _selected_file_paths(
    data_root: Path,
    file_version: tuple[int, int, int, int],
) -> list[tuple[Path, bool]]:
    """Every selected input for ``file_version``, before canonical ordering."""

    root = _canonical_dir(data_root, "product data directory")
    versioned_resource_dir = VERSIONED_RESOURCE_DIRS[file_version]
    versioned_root = _declared_resource_root(root, versioned_resource_dir)
    auxiliary_root = _declared_resource_root(root, AUXILIARY_RESOURCE_DIR)

    selected: list[tuple[Path, bool]] = []
    selected.extend(_collect_versioned_files(versioned_root))
    selected.extend(_collect_auxiliary_files(auxiliary_root))
    for relative in GRACE_MAP_PATHS:
        selected.append((_declared_path(root, relative, "grace map"), False))
    selected.append(
        (_declared_path(root, ENEMY_STATE_TABLES_PATH, "enemy-state capture"), False)
    )
    return selected


def _collect_versioned_files(versioned_root: Path) -> list[tuple[Path, bool]]:
    manifest_path, manifest = _read_manifest_snapshot(versioned_root, R4_SCHEMA)
    selected: list[tuple[Path, bool]] = [(manifest_path, True)]

    tables = manifest.get("tables")
    if not isinstance(tables, list):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{R4_SCHEMA}: manifest has no tables array",
        )
    declared_names = [
        _field_str(table, "name", R4_SCHEMA) for table in tables if isinstance(table, dict)
    ]
    if len(declared_names) != len(tables):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{R4_SCHEMA}: every table record must be an object with a name",
        )
    _require_exact_names(R4_SCHEMA, declared_names, R4_TABLE_NAMES)

    for name in R4_TABLE_NAMES:
        # R4 tables declare their blob under a nested ``file`` record, unlike
        # the auxiliary tables which name it directly.
        table = _r4_table(manifest, name)
        selected.append(
            (_read_declared_blob(versioned_root, _field(table, "file", name), name), True)
        )
    for section, key, label in R4_COMPANION_RECORDS:
        section_value = manifest.get(section)
        if not isinstance(section_value, dict):
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"{label}: manifest has no {section} section",
            )
        record = _field(section_value, key, section)
        selected.append((_read_declared_blob(versioned_root, record, label), True))
    return selected


def _collect_auxiliary_files(auxiliary_root: Path) -> list[tuple[Path, bool]]:
    manifest_path, manifest = _read_manifest_snapshot(auxiliary_root, AUXILIARY_SCHEMA)
    selected: list[tuple[Path, bool]] = [(manifest_path, False)]

    tables = manifest.get("tables")
    if not isinstance(tables, dict):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{AUXILIARY_SCHEMA}: manifest has no tables object",
        )
    _require_exact_names(
        AUXILIARY_SCHEMA,
        list(tables.keys()),
        [name for name, _ in AUXILIARY_TABLES],
    )
    for name, has_keys in AUXILIARY_TABLES:
        table = tables.get(name)
        if not isinstance(table, dict):
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"{name}: manifest has no auxiliary table {name!r}",
            )
        selected.append(
            (_read_declared_blob(auxiliary_root, _field(table, "file", name), name), False)
        )
        if has_keys:
            key_record = _field(table, "keys_file", name)
            selected.append(
                (_read_declared_blob(auxiliary_root, key_record, f"{name} keys"), False)
            )

    gate_section = manifest.get(AUXILIARY_GATE_SECTION)
    if not isinstance(gate_section, dict):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{AUXILIARY_GATE_SECTION}: manifest has no enemy parameter gate",
        )
    gate_record = _field(gate_section, AUXILIARY_GATE_KEY, AUXILIARY_GATE_SECTION)
    selected.append(
        (_read_declared_blob(auxiliary_root, gate_record, AUXILIARY_GATE_SECTION), False)
    )
    return selected


def _require_exact_names(
    schema: str,
    declared: list[str],
    expected: tuple[str, ...] | list[str],
) -> None:
    expected_names = list(expected)
    for name in declared:
        if name not in expected_names:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"{schema}: manifest declares table {name!r}, which the "
                "selected-bundle contract does not account for",
            )
    for name in expected_names:
        if name not in declared:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"{schema}: manifest is missing the consumed table {name!r}",
            )


def _field(value: Any, key: str, label: str) -> Any:
    if not isinstance(value, dict) or key not in value:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: missing field {key!r}",
        )
    return value[key]


def _field_str(value: Any, key: str, label: str) -> str:
    record = _field(value, key, label)
    if not isinstance(record, str):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: field {key!r} must be a string",
        )
    return record


def _r4_table(manifest: dict[str, Any], name: str) -> dict[str, Any]:
    for table in manifest.get("tables", []):
        if isinstance(table, dict) and table.get("name") == name:
            return table
    raise CoreServiceError(
        CoreErrorCode.RESOURCE_MISMATCH,
        f"{R4_SCHEMA}: manifest has no table named {name!r}",
    )


def _canonical_dir(path: Path, label: str) -> Path:
    try:
        canonical = Path(path).resolve(strict=True)
    except OSError as error:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: cannot resolve {path}: {error}",
        ) from error
    if not canonical.is_dir():
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: {canonical} is not a directory",
        )
    return canonical


def _declared_resource_root(data_root: Path, relative: str) -> Path:
    root = _canonical_dir(data_root / relative, relative)
    if not _is_within(root, data_root):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{relative}: canonical resource path escapes the product data directory",
        )
    return root


def _declared_path(root: Path, relative: str, label: str) -> Path:
    normal = Path()
    for part in Path(relative).parts:
        if part in ("", ".", "..") or Path(part).is_absolute() or ":" in part:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                f"{label}: unsafe declared path {relative!r}",
            )
        normal = normal / part
    if not normal.parts:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: empty declared path",
        )
    target = root / normal
    try:
        canonical = target.resolve(strict=True)
    except OSError as error:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: missing declared file {target}: {error}",
        ) from error
    if not _is_within(canonical, root):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: declared path escapes the resource root: {relative!r}",
        )
    return canonical


def _is_within(path: Path, root: Path) -> bool:
    try:
        path.relative_to(root)
    except ValueError:
        return False
    return True


def _read_manifest_snapshot(
    resource_root: Path, expected_schema: str
) -> tuple[Path, dict[str, Any]]:
    path = _declared_path(resource_root, "manifest.json", expected_schema)
    try:
        raw = path.read_bytes()
    except OSError as error:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{expected_schema}: cannot read {path}: {error}",
        ) from error
    try:
        document = json.loads(raw)
    except ValueError as error:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{expected_schema}: invalid JSON manifest at {path}: {error}",
        ) from error
    if not isinstance(document, dict):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{expected_schema}: manifest is not a JSON object",
        )
    schema = document.get("schema")
    if not isinstance(schema, str):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{expected_schema}: manifest has no schema",
        )
    if schema != expected_schema:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{expected_schema}: unsupported resource schema {schema!r}",
        )
    return path, document


def _read_declared_blob(root: Path, record: Any, label: str) -> Path:
    if not isinstance(record, dict):
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: declared record must be an object",
        )
    filename = _field_str(record, "filename", label)
    declared_size = _field(record, "size", label)
    if not isinstance(declared_size, int) or isinstance(declared_size, bool) or declared_size < 0:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: field 'size' must be a non-negative integer",
        )
    declared_sha = _field_str(record, "sha256", label).upper()
    path = _declared_path(root, filename, label)
    try:
        data = path.read_bytes()
    except OSError as error:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: cannot read {path}: {error}",
        ) from error
    if len(data) != declared_size:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: {path} size mismatch: expected {declared_size}, got {len(data)}",
        )
    digest = hashlib.sha256(data).hexdigest().upper()
    if digest != declared_sha:
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"{label}: {path} SHA-256 mismatch: {digest} != {declared_sha}",
        )
    return path


def _fold_bundle_digest(entries: list[tuple[str, bytes, bool]], *, versioned_only: bool) -> str:
    digest = hashlib.sha256()
    for relative, file_digest, versioned in entries:
        if versioned_only and not versioned:
            continue
        encoded = relative.encode("utf-8")
        digest.update(struct.pack("<I", len(encoded)))
        digest.update(encoded)
        digest.update(file_digest)
    return digest.hexdigest()


def resolve_selected_generation_bundle(
    data_root: Path,
    file_version: Any,
) -> SelectedGenerationBundle:
    """Resolve the selected generation resource bundle for one exact version.

    The executable version is validated first: an unregistered version fails
    closed before the data root is read, so an unknown identity can never be
    resolved against fallback content.
    """

    version = _require_explicit_file_version(file_version)
    versioned_resource_dir = VERSIONED_RESOURCE_DIRS.get(version)
    if versioned_resource_dir is None:
        display = dotted_file_version(version)
        raise CoreServiceError(
            CoreErrorCode.RESOURCE_MISMATCH,
            f"no offline generation resource for executable version {display}",
        )

    root = _canonical_dir(Path(data_root), "product data directory")
    selected = _selected_file_paths(root, version)

    by_relative: dict[str, tuple[bytes, bool]] = {}
    for path, versioned in selected:
        relative = path.relative_to(root).as_posix()
        if relative in by_relative:
            continue
        by_relative[relative] = (hashlib.sha256(path.read_bytes()).digest(), versioned)

    ordered = [
        (relative, by_relative[relative][0], by_relative[relative][1])
        for relative in sorted(by_relative, key=lambda item: (item.lower(), item))
    ]
    return SelectedGenerationBundle(
        file_version=version,
        versioned_resource_dir=versioned_resource_dir,
        bundle_digest=_fold_bundle_digest(ordered, versioned_only=False),
        versioned_digest=_fold_bundle_digest(ordered, versioned_only=True),
        files=tuple(relative for relative, _, _ in ordered),
    )


@dataclass(frozen=True, slots=True)
class ResolvedGenerationContext:
    """Production generation identity bound to one exact installed version."""

    product_version: str
    game_profile: str
    game_file_version: tuple[int, int, int, int]
    versioned_resource_dir: str
    bundle_digest: str
    versioned_digest: str
    resources_digest: str
    algorithm_version: str
    policy_version: str
    seed_accelerator_abi: int | None
    seed_accelerator_build_id: str | None
    legacy_context_digest: str
    context_digest: str

    @property
    def schema(self) -> str:
        return RESOLVED_CONTEXT_SCHEMA

    @property
    def dotted_game_file_version(self) -> str:
        return dotted_file_version(self.game_file_version)

    @classmethod
    def capture(
        cls,
        *,
        file_version: Any,
        game_profile: str = SUPPORTED_GAME_PROFILE,
        data_root: Path | None = None,
        accelerator: tuple[int | None, str | None] | None = None,
        accelerator_identity: tuple[int, str] | None = None,
    ) -> "ResolvedGenerationContext":
        """Capture the version-bound production identity for ``data_root``.

        ``file_version`` must be supplied explicitly; a missing or unregistered
        version fails closed and there is no default fallback.

        ``accelerator`` lets a caller state the accelerator identity explicitly
        instead of probing the host DLL. ``None`` (the default) probes the loaded
        accelerator, matching the shipped worker. Pass ``(None, None)`` to
        capture the accelerator-free identity the pinned Rust goldens use.
        """

        version = _require_explicit_file_version(file_version)
        root = default_data_root() if data_root is None else Path(data_root)
        bundle = resolve_selected_generation_bundle(root, version)
        return cls.from_bundle(
            game_profile=game_profile,
            file_version=version,
            data_root=root,
            bundle=bundle,
            accelerator=accelerator,
            accelerator_identity=accelerator_identity,
        )

    @classmethod
    def from_bundle(
        cls,
        *,
        game_profile: str,
        file_version: Any,
        data_root: Path,
        bundle: SelectedGenerationBundle,
        accelerator: tuple[int | None, str | None] | None = None,
        accelerator_identity: tuple[int, str] | None = None,
    ) -> "ResolvedGenerationContext":
        """Build the identity from an already-resolved selected bundle.

        A bundle resolved for a different version is refused, so the identity
        can never describe a bundle other than the one it names.
        """

        version = _require_explicit_file_version(file_version)
        if bundle.file_version != version:
            raise CoreServiceError(
                CoreErrorCode.RESOURCE_MISMATCH,
                "selected bundle is for "
                f"{dotted_file_version(bundle.file_version)} but the context was "
                f"requested for {dotted_file_version(version)}",
            )

        if accelerator is not None:
            abi, build_id = accelerator
            abi = int(abi) if abi is not None else None
            build_id = str(build_id) if build_id is not None else None
        else:
            identity = accelerator_identity or seed_accelerator_identity()
            abi = int(identity[0]) if identity else None
            build_id = str(identity[1]) if identity else None
        resources_digest = runtime_resource_digest(str(Path(data_root).resolve()))

        legacy_canonical = _legacy_canonical_payload(
            product_version=PRODUCT_VERSION,
            game_profile=game_profile,
            resources_digest=resources_digest,
            algorithm_version=GENERATION_ALGORITHM_VERSION,
            policy_version=OPERATION_POLICY_VERSION,
            seed_accelerator_abi=abi,
            seed_accelerator_build_id=build_id,
        )
        legacy_digest = hashlib.sha256(legacy_canonical.encode("utf-8")).hexdigest()

        canonical = _resolved_canonical_payload(
            product_version=PRODUCT_VERSION,
            game_profile=game_profile,
            game_file_version=version,
            versioned_resource_dir=bundle.versioned_resource_dir,
            bundle_digest=bundle.bundle_digest,
            versioned_digest=bundle.versioned_digest,
            resources_digest=resources_digest,
            algorithm_version=GENERATION_ALGORITHM_VERSION,
            policy_version=OPERATION_POLICY_VERSION,
            seed_accelerator_abi=abi,
            seed_accelerator_build_id=build_id,
        )
        return cls(
            product_version=PRODUCT_VERSION,
            game_profile=game_profile,
            game_file_version=version,
            versioned_resource_dir=bundle.versioned_resource_dir,
            bundle_digest=bundle.bundle_digest,
            versioned_digest=bundle.versioned_digest,
            resources_digest=resources_digest,
            algorithm_version=GENERATION_ALGORITHM_VERSION,
            policy_version=OPERATION_POLICY_VERSION,
            seed_accelerator_abi=abi,
            seed_accelerator_build_id=build_id,
            legacy_context_digest=legacy_digest,
            context_digest=hashlib.sha256(canonical.encode("utf-8")).hexdigest(),
        )

    def canonical_payload(self) -> str:
        """Exact bytes hashed into :attr:`context_digest`."""

        return _resolved_canonical_payload(
            product_version=self.product_version,
            game_profile=self.game_profile,
            game_file_version=self.game_file_version,
            versioned_resource_dir=self.versioned_resource_dir,
            bundle_digest=self.bundle_digest,
            versioned_digest=self.versioned_digest,
            resources_digest=self.resources_digest,
            algorithm_version=self.algorithm_version,
            policy_version=self.policy_version,
            seed_accelerator_abi=self.seed_accelerator_abi,
            seed_accelerator_build_id=self.seed_accelerator_build_id,
        )

    def to_payload(self) -> dict[str, Any]:
        """Production handshake payload, matching ``to_payload`` in Rust."""

        return {
            "product_version": self.product_version,
            "game_profile": self.game_profile,
            "resources_digest": self.resources_digest,
            "algorithm_version": self.algorithm_version,
            "policy_version": self.policy_version,
            "seed_accelerator_abi": self.seed_accelerator_abi,
            "seed_accelerator_build_id": self.seed_accelerator_build_id,
            "context_digest": self.context_digest,
            "game_file_version": self.dotted_game_file_version,
            "versioned_resource_dir": self.versioned_resource_dir,
            "bundle_digest": self.bundle_digest,
            "versioned_digest": self.versioned_digest,
            "legacy_context_digest": self.legacy_context_digest,
            "production_authority": True,
        }


@dataclass(frozen=True, slots=True)
class LegacyGenerationContext:
    """Opt-in, visibly non-production pre-version identity.

    This reproduces the shipped seven-field digest for diagnostics only. It is a
    different type from :class:`ResolvedGenerationContext` and can never
    authorize a candidate, cache, or resume.
    """

    product_version: str
    game_profile: str
    resources_digest: str
    algorithm_version: str
    policy_version: str
    seed_accelerator_abi: int | None
    seed_accelerator_build_id: str | None
    context_digest: str

    @property
    def schema(self) -> str:
        return LEGACY_CONTEXT_SCHEMA

    def to_payload(self) -> dict[str, Any]:
        return {
            "product_version": self.product_version,
            "game_profile": self.game_profile,
            "resources_digest": self.resources_digest,
            "algorithm_version": self.algorithm_version,
            "policy_version": self.policy_version,
            "seed_accelerator_abi": self.seed_accelerator_abi,
            "seed_accelerator_build_id": self.seed_accelerator_build_id,
            "legacy_context_digest": self.context_digest,
            "context_digest": self.context_digest,
            "production_authority": False,
        }


def capture_legacy_context(
    *,
    game_profile: str = SUPPORTED_GAME_PROFILE,
    data_root: Path | None = None,
    accelerator: tuple[int | None, str | None] | None = None,
) -> LegacyGenerationContext:
    """Capture the explicit pre-version identity for proof/diagnostic use."""

    root = default_data_root() if data_root is None else Path(data_root)
    if accelerator is not None:
        abi, build_id = accelerator
        abi = int(abi) if abi is not None else None
        build_id = str(build_id) if build_id is not None else None
    else:
        identity = seed_accelerator_identity()
        abi = int(identity[0]) if identity else None
        build_id = str(identity[1]) if identity else None
    resources_digest = runtime_resource_digest(str(root.resolve()))
    canonical = _legacy_canonical_payload(
        product_version=PRODUCT_VERSION,
        game_profile=game_profile,
        resources_digest=resources_digest,
        algorithm_version=GENERATION_ALGORITHM_VERSION,
        policy_version=OPERATION_POLICY_VERSION,
        seed_accelerator_abi=abi,
        seed_accelerator_build_id=build_id,
    )
    return LegacyGenerationContext(
        product_version=PRODUCT_VERSION,
        game_profile=game_profile,
        resources_digest=resources_digest,
        algorithm_version=GENERATION_ALGORITHM_VERSION,
        policy_version=OPERATION_POLICY_VERSION,
        seed_accelerator_abi=abi,
        seed_accelerator_build_id=build_id,
        context_digest=hashlib.sha256(canonical.encode("utf-8")).hexdigest(),
    )


def default_data_root() -> Path:
    """Shipped offline data root, without selecting any version."""

    return Path(__file__).resolve().parent / "data"


@lru_cache(maxsize=4)
def cached_resolved_context(
    file_version: tuple[int, int, int, int],
    data_root_text: str | None = None,
) -> ResolvedGenerationContext:
    """Memoized resolution for one explicit version and data root."""

    root = Path(data_root_text) if data_root_text else None
    return ResolvedGenerationContext.capture(file_version=file_version, data_root=root)


__all__ = [
    "AUXILIARY_RESOURCE_DIR",
    "AUXILIARY_SCHEMA",
    "AUXILIARY_TABLES",
    "ENEMY_STATE_TABLES_PATH",
    "GRACE_MAP_PATHS",
    "LEGACY_CONTEXT_SCHEMA",
    "LegacyGenerationContext",
    "PRODUCT_VERSION",
    "R4_COMPANION_RECORDS",
    "R4_SCHEMA",
    "R4_TABLE_NAMES",
    "RESOLVED_CONTEXT_SCHEMA",
    "ResolvedGenerationContext",
    "SelectedGenerationBundle",
    "VERSIONED_RESOURCE_DIRS",
    "cached_resolved_context",
    "capture_legacy_context",
    "default_data_root",
    "dotted_file_version",
    "normalize_file_version",
    "resolve_selected_generation_bundle",
]
