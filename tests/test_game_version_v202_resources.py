"""PC v2.02 offline generation resource registration.

These tests pin three things: the v2.02 profile stays fail-closed, the versioned
resource carries the accepted retained capture rather than a silent copy of the
shipped tables, and registering the version enables no live path.
"""

from __future__ import annotations

import hashlib
import json
import struct
from pathlib import Path

import pytest

from nioh3_scroll_editor.game_compatibility import SUPPORTED_GAME_VERSIONS
from nioh3_scroll_editor.effect_generation_tables import (
    effect_generation_tables_for_game_version,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.r4_finalizer_engine import (
    load_default_r4_finalizer_engine,
    load_r4_finalizer_engine_for_game_version,
)
from nioh3_scroll_editor.r4_finalizer_resource import (
    ALIASED_GAME_VERSIONS,
    DEFAULT_RESOURCE_ROOT,
    VERSION_RESOURCE_ROOTS,
    load_default_r4_finalizer_resource,
    load_r4_finalizer_resource_for_version,
    resource_root_for_game_version,
)

REPO = Path(__file__).resolve().parents[1]
PROFILE = REPO / "nioh3_scroll_editor/data/game_versions/pc_v2_02.json"
COMPARISON = (
    REPO
    / "tests/fixtures/game-version-update-20260919"
    / "reports/resource-comparison-v2.02-ng3.json"
)

V202 = (2, 0, 2, 0)
EXE_SHA = "E22C4A635E4EC1E27A177B76E27D7F6A637F426C0ED3928B60F5693BC52AE130"

# Payloads that the retained lane comparison proved byte-equal to the shipped
# resource.  They must stay equal, because the resource decision depends on it.
EQUAL_FILES = (
    "bonus_curve/index.bin",
    "bonus_curve/rows.bin",
    "tables/category.bin",
    "tables/category_count_multiplier.bin",
    "tables/effect.bin",
    "tables/effect_group.bin",
    "tables/level_curve.bin",
    "tables/rarity_roll.bin",
    "tables/special_context.bin",
)


def _sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest().upper()


def test_v202_profile_is_present_and_fail_closed() -> None:
    profile = json.loads(PROFILE.read_text(encoding="utf-8"))
    assert profile["schema"] == "nioh3-game-version-research-profile/v1"
    assert profile["profile_id"] == "pc_v2_02"
    assert profile["display_version"] == "PC v2.02"
    assert profile["file_version"] == list(V202)
    assert profile["approval_status"] == "candidate"
    assert profile["product_enablement_allowed"] is False
    assert profile["gates"]["product_enablement_allowed"] is False
    assert profile["gates"]["save_layout_validated"] is False
    assert profile["provenance"]["executable_sha256"] == EXE_SHA
    assert len(profile["text_sites"]) == 22
    assert len(profile["rdata_sites"]) == 6


def test_live_compatibility_registry_is_still_closed() -> None:
    assert V202 not in SUPPORTED_GAME_VERSIONS
    assert (2, 0, 1, 0) in SUPPORTED_GAME_VERSIONS


def test_registered_version_does_not_change_the_default_resource() -> None:
    assert resource_root_for_game_version(V202) == VERSION_RESOURCE_ROOTS[V202]
    for aliased in ALIASED_GAME_VERSIONS:
        assert resource_root_for_game_version(aliased) == DEFAULT_RESOURCE_ROOT
    assert load_default_r4_finalizer_resource().root == DEFAULT_RESOURCE_ROOT


def test_v202_resource_loads_and_matches_the_accepted_comparison() -> None:
    bundle = load_r4_finalizer_resource_for_version(V202)
    assert bundle.manifest["game_version"] == "PC v2.02"
    comparison = json.loads(COMPARISON.read_text(encoding="utf-8"))
    recorded = {
        item["filename"]: item["target"]["sha256"]
        for item in comparison["r4"]["files"]
        # The captured progress vector is save-dependent; the shipped baseline
        # vector is deliberate and is asserted in its own test below.
        if item["kind"] == "static"
    }
    target = resource_root_for_game_version(V202)
    for relative, digest in recorded.items():
        path = target / relative
        assert path.is_file(), relative
        assert _sha256(path) == digest, relative
    comparison = json.loads(COMPARISON.read_text(encoding="utf-8"))
    assert comparison["gates"]["static_generation_resources_equal"] is False
    assert comparison["gates"]["playthrough_context_equal"] is True
    assert comparison["gates"]["product_enablement_allowed"] is False


def test_unchanged_payloads_are_byte_equal_to_the_shipped_resource() -> None:
    target = resource_root_for_game_version(V202)
    for relative in EQUAL_FILES:
        assert _sha256(target / relative) == _sha256(DEFAULT_RESOURCE_ROOT / relative), (
            relative
        )


def test_item_table_carries_the_single_documented_field_change() -> None:
    new = load_r4_finalizer_resource_for_version(V202).table("item")
    old = load_default_r4_finalizer_resource().table("item")
    assert new.row_count == old.row_count == 3362
    new_row = list(new.rows())[3358]
    old_row = list(old.rows())[3358]
    assert struct.unpack_from("<I", old_row, 0x84)[0] == 0x380
    assert struct.unpack_from("<I", new_row, 0x84)[0] == 0x0
    # Everything else in that row is unchanged.
    assert new_row[:0x84] == old_row[:0x84]
    assert new_row[0x88:] == old_row[0x88:]


def test_optional_multiplier_growth_and_changed_key_are_integrated() -> None:
    new = load_r4_finalizer_resource_for_version(V202).table("optional_multiplier")
    old = load_default_r4_finalizer_resource().table("optional_multiplier")
    assert old.row_count == 2951
    assert new.row_count == 2954

    def by_key(table):
        result = {}
        for row in table.rows():
            result[struct.unpack_from("<I", row, 0x14)[0]] = struct.unpack_from(
                "<f", row, 0x18
            )[0]
        return result

    new_keys, old_keys = by_key(new), by_key(old)
    for key in (0x3472, 0xAA65, 0xD56F):
        assert key in new_keys and key not in old_keys
    def keyed_rows(table):
        result = {}
        for row in table.rows():
            result[struct.unpack_from("<I", row, 0x14)[0]] = row
        return result

    new_rows, old_rows = keyed_rows(new), keyed_rows(old)
    # The multiplier float the offline index consumes is unchanged...
    assert struct.unpack_from("<f", old_rows[0xD7C3], 0x18)[0] == 1.0
    assert struct.unpack_from("<f", new_rows[0xD7C3], 0x18)[0] == 1.0
    # ...and the raw value the game reads as the recommended level did change.
    assert struct.unpack_from("<I", old_rows[0xD7C3], 0x10)[0] == 1400
    assert struct.unpack_from("<I", new_rows[0xD7C3], 0x10)[0] == 600


def test_playthrough_progress_keeps_the_baseline_vector() -> None:
    """Progress is save-dependent; the versioned resource keeps the baseline."""

    new = load_r4_finalizer_resource_for_version(V202)
    old = load_default_r4_finalizer_resource()
    assert new.playthrough_progress(3) == old.playthrough_progress(3)
    assert _sha256(
        resource_root_for_game_version(V202) / "globals/playthrough_progress.bin"
    ) == _sha256(DEFAULT_RESOURCE_ROOT / "globals/playthrough_progress.bin")


def test_generation_entrypoint_uses_the_v202_resource() -> None:
    """The wired entrypoints must exercise the new resource, not only the loader."""

    shipped = load_default_effect_generation_tables()
    candidate = effect_generation_tables_for_game_version(V202)
    assert len(shipped.optional_multipliers_by_key) == 2951
    assert len(candidate.optional_multipliers_by_key) == 2954
    for key in (0x3472, 0xAA65, 0xD56F):
        assert key not in shipped.optional_multipliers_by_key
        assert key in candidate.optional_multipliers_by_key
    assert shipped.optional_multipliers_by_key[0xD7C3].multiplier == 1.0
    assert candidate.optional_multipliers_by_key[0xD7C3].multiplier == 1.0
    assert candidate.resource.root != shipped.resource.root


def test_version_bound_engine_is_built_from_the_versioned_resource() -> None:
    engine = load_r4_finalizer_engine_for_game_version(V202)
    assert engine.tables.resource.manifest["game_version"] == "PC v2.02"
    assert load_default_r4_finalizer_engine().tables.resource.manifest[
        "game_version"
    ] == "PC v2.00.02"


def test_unknown_version_fails_closed_at_the_entrypoint() -> None:
    with pytest.raises(ValueError):
        effect_generation_tables_for_game_version((2, 1, 0, 0))
