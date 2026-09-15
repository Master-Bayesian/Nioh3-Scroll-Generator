"""Cross-language parity gate for the M2.1 effect-generation slice.

Two Rust development emitters run against the retained Python reference:

* `crates/nioh3-domain/examples/effect_vectors.rs` decodes the shipped
  header-prefixed tables through `nioh3_domain::effect`.
* `crates/nioh3-data/examples/effect_sequence_vectors.rs` loads the same
  resource through the real production adapter
  (`nioh3_data::load_effect_resource`) and generates the three ordinary NG3
  sequences.

Every emitted row is reproduced from the retained reference modules and
compared exactly; the native fixture records in
`test_fixtures/effect_sequence_vectors.json` independently anchor the sequence
fields to captured game bytes. The gate fails closed on any mismatch.
"""
from __future__ import annotations

import json
import os
from pathlib import Path
import struct
import subprocess
import unittest

from nioh3_scroll_editor.effect_generation_tables import (
    SCROLL_RECORD_TYPES,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.effect_sequence import (
    generate_challenge_attempt_count,
    generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity4_stage_one_effect_sequence,
    generate_ng3_rarity5_effect_sequence,
)
from nioh3_scroll_editor.grace_map import load_grace_output_map


ROOT = Path(__file__).resolve().parents[2]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
RESOURCE = DATA_ROOT / "r4_finalizer" / "pc_v2_00_02" / "resource_v1"
MANIFEST = json.loads((RESOURCE / "manifest.json").read_text(encoding="utf-8"))
VECTORS = json.loads(
    (ROOT / "test_fixtures" / "effect_sequence_vectors.json").read_text(
        encoding="utf-8"
    )
)
SEED_SWEEP = [
    0,
    1,
    2,
    241_719_428,
    0x0FFF_FFFF,
    82_212_268,
    183_696_634,
    0xFFFF_FFFF,
    0xFFFF_FFFE,
    0x8000_0000,
] + [(index * 2_654_435_761) & 0xFFFF_FFFF for index in range(512)]

LEVEL = 180
NG3_RECORD_TYPE = 0xE604
EFFECT_START = 0x34
EFFECT_STRIDE = 0x18
SEQUENCE_SEEDS = [
    0,
    1,
    2,
    2965,
    240_348_265,
    6_096_970,
    74_063_692,
    82_212_268,
    183_696_634,
    241_719_428,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
] + [(index * 2_654_435_761) & 0xFFFF_FFFF for index in range(96)]
SEQUENCE_PATHS = ("r3", "r4_stage_one", "r5")
LEVEL_SWEEP = [1, 30, 90, 150, 180, 300, 500, 700, 65_535]
LEVEL_SWEEP_SEEDS = [
    0,
    1,
    2,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFF,
    (5 * 2_654_435_761) & 0xFFFF_FFFF,
    (97 * 2_654_435_761) & 0xFFFF_FFFF,
]


def fnv1a64(payload: bytes) -> int:
    value = 0xCBF29CE484222325
    for byte in payload:
        value ^= byte
        value = (value * 0x100000001B3) & 0xFFFF_FFFF_FFFF_FFFF
    return value


def run_example(manifest: Path, example: str, *arguments: str) -> list[str]:
    """Run one Rust development emitter and return its tabbed output lines."""

    target = os.environ.get(
        "CARGO_TARGET_DIR", str(ROOT / ".codex_tmp" / "m2_effect_parity_target")
    )
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--locked",
            "--offline",
            "--quiet",
            "--manifest-path",
            str(manifest),
            "--example",
            example,
            "--",
            *arguments,
        ],
        cwd=ROOT,
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            f"Rust emitter {example} failed: "
            + (completed.stderr.strip() or completed.stdout.strip())
        )
    return [line for line in completed.stdout.splitlines() if line and "\t" in line]


def rust_lines() -> list[str]:
    return run_example(
        ROOT / "crates" / "nioh3-domain" / "Cargo.toml",
        "effect_vectors",
        str(DATA_ROOT),
    )


def sequence_rust_lines() -> list[str]:
    return run_example(
        ROOT / "crates" / "nioh3-data" / "Cargo.toml",
        "effect_sequence_vectors",
        str(DATA_ROOT),
    )


def reference_lines() -> list[str]:
    index = load_default_effect_generation_tables()
    tables = {entry["name"]: entry for entry in MANIFEST["tables"]}
    lines: list[str] = []
    for name in (
        "item",
        "effect_group",
        "category",
        "category_count_multiplier",
        "effect",
    ):
        entry = tables[name]
        lines.append(f"table\t{name}\t{entry['row_size']}\t{entry['row_count']}")
    for record_type in SCROLL_RECORD_TYPES:
        item = index.items_by_record_type[record_type]
        lines.append(
            "item\t{}\t{}\t{}\t{}\t{}".format(
                record_type,
                item.field_154,
                item.field_15c,
                item.mode,
                item.candidate_item_flags,
            )
        )
    for key in sorted(index.groups_by_key):
        group = index.groups_by_key[key]
        lines.append(
            "group\t{}\t{}\t{}\t{}".format(
                group.group_key,
                group.category_key,
                group.conflict_mask_0,
                group.conflict_mask_1,
            )
        )
    for key in sorted(index.categories_by_key):
        category = index.categories_by_key[key]
        capacities = ",".join(str(value) for value in category.rarity_capacities)
        lines.append(
            "category\t{}\t{}\t{}\t{}\t{}".format(
                category.category_key,
                capacities,
                category.mode12_lottery_weight,
                category.mode12_capacity,
                category.mode12_count_multiplier_key,
            )
        )
    for key in sorted(index.category_count_multipliers_by_key):
        entry = index.category_count_multipliers_by_key[key]
        bits = ",".join(
            format(struct.unpack("<I", struct.pack("<f", value))[0], "08x")
            for value in entry.multipliers
        )
        lines.append(f"ccmult\t{entry.lookup_key}\t{bits}")
    for effect_id in sorted(index.effects_by_id):
        effect = index.effects_by_id[effect_id]
        digest = fnv1a64(struct.pack("<64H", *effect.lottery_weights))
        lines.append(
            "effect\t{}\t{}\t{}\t{}\t{}\t{}\t{:016x}".format(
                effect.effect_id,
                effect.group_key,
                effect.flags,
                effect.normalization_flags,
                effect.progress_threshold,
                effect.alternate_threshold,
                digest,
            )
        )
    for seed in SEED_SWEEP:
        lines.append(f"challenge\t{seed}\t{generate_challenge_attempt_count(seed)}")
    return lines


def describe_sequence(path: str, seed: int, result) -> str:
    """Mirror `effect_sequence_vectors.rs`'s `describe` field for field."""

    effects = ",".join(
        "{}:{}:{:04X}:{}:{:02X}:{:02X}:{}:{}:{:04X}".format(
            effect.slot,
            effect.source_index,
            effect.effect_id,
            effect.roll_percent,
            effect.category_and_flags,
            effect.effect_flags,
            effect.candidate_count,
            effect.resolved_value,
            effect.prefix_word,
        )
        for effect in result.effects
    )
    promoted = ",".join(str(index) for index in result.promoted_source_indexes)
    return "seq\t{}\t{}\t{}\t{:04X}\t{}\t{}\t{}\t{}\t{}\t{:08X}\t{}".format(
        path,
        seed,
        result.level,
        result.record_type,
        result.rarity,
        result.playthrough,
        str(result.terminal_is_special).lower(),
        promoted,
        result.random_draws,
        result.final_rng_state,
        effects,
    )


def sequence_reference_lines() -> list[str]:
    index = load_default_effect_generation_tables()
    stage_one_map = load_grace_output_map(rarity=4)
    grace_map = load_grace_output_map(rarity=5)
    lines: list[str] = []
    for rarity in (3, 4, 5):
        capacities = index.category_capacities(
            record_type=NG3_RECORD_TYPE,
            rarity=rarity,
        )
        lines.append(
            "capacities\t{:04X}\t{}\t{}".format(
                NG3_RECORD_TYPE,
                rarity,
                ",".join(str(value) for value in capacities),
            )
        )
    for seed in SEQUENCE_SEEDS:
        lines.append(
            describe_sequence(
                "r3",
                seed,
                generate_ng3_rarity3_effect_sequence(seed, level=LEVEL),
            )
        )
        lines.append(
            describe_sequence(
                "r4_stage_one",
                seed,
                generate_ng3_rarity4_stage_one_effect_sequence(
                    seed,
                    level=LEVEL,
                    special_mapping=stage_one_map,
                ),
            )
        )
        lines.append(
            describe_sequence(
                "r5",
                seed,
                generate_ng3_rarity5_effect_sequence(
                    seed,
                    level=LEVEL,
                    grace_mapping=grace_map,
                ),
            )
        )
    # Level block. The emitter runs levels outer, seeds inner, then the three
    # paths; this loop must keep that exact order.
    for level in LEVEL_SWEEP:
        for seed in LEVEL_SWEEP_SEEDS:
            lines.append(
                describe_sequence(
                    "r3",
                    seed,
                    generate_ng3_rarity3_effect_sequence(seed, level=level),
                )
            )
            lines.append(
                describe_sequence(
                    "r4_stage_one",
                    seed,
                    generate_ng3_rarity4_stage_one_effect_sequence(
                        seed,
                        level=level,
                        special_mapping=stage_one_map,
                    ),
                )
            )
            lines.append(
                describe_sequence(
                    "r5",
                    seed,
                    generate_ng3_rarity5_effect_sequence(
                        seed,
                        level=level,
                        grace_mapping=grace_map,
                    ),
                )
            )
    return lines


def row_fields(line: str) -> list[str]:
    """Split one `seq` row: kind, path, seed, level, identity, effects."""

    return line.split("\t")


def resolved_values(fields: list[str]) -> tuple[int, ...]:
    return tuple(int(effect.split(":")[7]) for effect in fields[11].split(","))


def without_level_and_resolved(fields: list[str]) -> tuple[str, ...]:
    """Row signature excluding the level column and per-effect resolved values.

    Everything else in the row (path, seed, record type, rarity, playthrough,
    terminal flag, promotion, draw count, final state, slot order, effect IDs,
    rolls, category/flags, candidate counts and prefix words) must be invariant
    when only the level changes.
    """

    effects = tuple(
        ":".join(effect_fields[:7] + effect_fields[8:])
        for effect_fields in (
            effect.split(":") for effect in fields[11].split(",")
        )
    )
    return (*fields[1:3], *fields[4:11], *effects)


def native_effect_fields(record: bytes, count: int) -> list[tuple[int, ...]]:
    """Parse the serialized effect slots the emitters describe."""

    fields: list[tuple[int, ...]] = []
    for index in range(count):
        offset = EFFECT_START + index * EFFECT_STRIDE
        fields.append(
            (
                struct.unpack_from("<I", record, offset)[0],
                struct.unpack_from("<I", record, offset + 4)[0],
                struct.unpack_from("<i", record, offset + 8)[0],
                record[offset + 0x0C],
                record[offset + 0x0D],
                record[offset + 0x0E],
            )
        )
    return fields


def generated_effect_fields(result) -> list[tuple[int, ...]]:
    return [
        (
            effect.prefix_word,
            effect.effect_id,
            effect.resolved_value,
            effect.roll_percent,
            effect.category_and_flags,
            effect.effect_flags,
        )
        for effect in result.effects
    ]


class EffectTableParityTests(unittest.TestCase):
    """Domain decoding parity for the shipped header-prefixed tables."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.reference = [
            line for line in reference_lines() if not line.startswith("probe\t")
        ]
        cls.rust = [line for line in rust_lines() if not line.startswith("probe\t")]

    def test_every_reference_row_matches_the_rust_decoder(self) -> None:
        self.assertGreater(len(self.reference), 5000)
        self.assertEqual(len(self.reference), len(self.rust))
        mismatches = [
            (reference, actual)
            for reference, actual in zip(self.reference, self.rust)
            if reference != actual
        ]
        self.assertEqual(mismatches[:5], [])

    def test_group_and_category_indexes_are_complete(self) -> None:
        groups = [line for line in self.rust if line.startswith("group\t")]
        categories = [line for line in self.rust if line.startswith("category\t")]
        effects = [line for line in self.rust if line.startswith("effect\t")]
        self.assertEqual(len(groups), 1152)
        self.assertEqual(len(categories), 17)
        self.assertEqual(len(effects), 3609)

    def test_challenge_sweep_covers_edges_and_a_broader_seed_set(self) -> None:
        challenge = [line for line in self.rust if line.startswith("challenge\t")]
        self.assertEqual(len(challenge), len(SEED_SWEEP))
        counts = {}
        for line in challenge:
            _, seed, count = line.split("\t")
            counts[int(seed)] = int(count)
            self.assertGreaterEqual(int(count), 4)
            self.assertLessEqual(int(count), 7)
        self.assertEqual(counts[1], 4)
        self.assertEqual(counts[0x0FFF_FFFF], 7)


class EffectSequenceParityTests(unittest.TestCase):
    """Production-adapter sequence parity over the fixed and swept seeds."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.reference = sequence_reference_lines()
        cls.rust = sequence_rust_lines()
        cls.by_key: dict[tuple[str, int, int], list[str]] = {}
        for line in cls.rust:
            parts = line.split("\t")
            if parts[0] == "seq":
                cls.by_key[(parts[1], int(parts[2]), int(parts[3]))] = parts

    def row(self, path: str, seed: int, level: int = LEVEL) -> list[str]:
        return self.by_key[(path, seed, level)]

    def test_every_sequence_row_matches_the_reference(self) -> None:
        expected = 3 + 3 * (
            len(SEQUENCE_SEEDS) + len(LEVEL_SWEEP) * len(LEVEL_SWEEP_SEEDS)
        )
        self.assertEqual(len(self.rust), expected)
        self.assertEqual(len(self.reference), len(self.rust))
        mismatches = [
            (reference, actual)
            for reference, actual in zip(self.reference, self.rust)
            if reference != actual
        ]
        self.assertEqual(mismatches[:3], [])

    def test_capacity_vectors_match_for_every_ordinary_rarity(self) -> None:
        capacities = [line for line in self.rust if line.startswith("capacities\t")]
        self.assertEqual(len(capacities), 3)
        for line in capacities:
            _, record_type, rarity, values = line.split("\t")
            self.assertEqual(int(record_type, 16), NG3_RECORD_TYPE)
            entries = [int(value) for value in values.split(",")]
            self.assertEqual(len(entries), 32)
            self.assertIn(int(rarity), (3, 4, 5))
            self.assertTrue(any(entries))

    def test_every_fixed_seed_covers_all_three_paths(self) -> None:
        for seed in SEQUENCE_SEEDS:
            for path in SEQUENCE_PATHS:
                self.assertIn((path, seed, LEVEL), self.by_key, f"{path} seed {seed}")

    def test_level_sweep_covers_every_level_seed_and_path(self) -> None:
        self.assertEqual(len(LEVEL_SWEEP), 9)
        self.assertEqual(len(LEVEL_SWEEP_SEEDS), 8)
        for level in LEVEL_SWEEP:
            for seed in LEVEL_SWEEP_SEEDS:
                for path in SEQUENCE_PATHS:
                    self.assertIn(
                        (path, seed, level),
                        self.by_key,
                        f"{path} seed {seed} level {level}",
                    )
        self.assertEqual(
            len(LEVEL_SWEEP) * len(LEVEL_SWEEP_SEEDS) * len(SEQUENCE_PATHS),
            216,
        )

    def test_only_the_level_and_resolved_values_follow_the_level(self) -> None:
        """Level invariance of identity, order, draws and state."""

        for seed in LEVEL_SWEEP_SEEDS:
            for path in SEQUENCE_PATHS:
                baseline = without_level_and_resolved(self.row(path, seed, 1))
                for level in LEVEL_SWEEP[1:]:
                    self.assertEqual(
                        without_level_and_resolved(self.row(path, seed, level)),
                        baseline,
                        f"{path} seed {seed} changed beyond level/resolved at {level}",
                    )

    def test_level_sweep_changes_resolved_values(self) -> None:
        """A level-scaling defect cannot hide behind the level-180 matrix.

        Measured on the shipped pc_v2_00_02 tables: 5 of the 24 level groups
        are level-sensitive and move through exactly 7 distinct resolved tuples
        (the seven levels below the 500 clamp); the other 19 keep their base
        value because the recovered formula has a zero-width roll spread for
        those rows. The distribution is pinned so a table or reference change
        has to be re-measured instead of silently weakening this gate.
        """

        distribution: dict[int, int] = {}
        for seed in LEVEL_SWEEP_SEEDS:
            for path in SEQUENCE_PATHS:
                distinct = {
                    resolved_values(self.row(path, seed, level))
                    for level in LEVEL_SWEEP
                }
                distribution[len(distinct)] = distribution.get(len(distinct), 0) + 1
        self.assertEqual(distribution, {1: 19, 7: 5})

    def test_levels_above_the_curve_clamp_reduce_to_the_level_500_row(self) -> None:
        """`resolved_base_value` clamps at 500 before reading the curve.

        Levels 700 and 65535 therefore keep their verbatim level column but
        reproduce the level-500 effect block exactly; the reference rejects
        nothing in this range and emits no unsupported value.
        """

        for seed in LEVEL_SWEEP_SEEDS:
            for path in SEQUENCE_PATHS:
                clamped = self.row(path, seed, 500)
                for level in (700, 65_535):
                    above = self.row(path, seed, level)
                    self.assertEqual(int(above[3]), level)
                    self.assertEqual(
                        without_level_and_resolved(above),
                        without_level_and_resolved(clamped),
                    )
                    self.assertEqual(
                        resolved_values(above),
                        resolved_values(clamped),
                        f"{path} seed {seed} level {level} is not clamped",
                    )

    def test_promotion_and_draw_accounting_are_exact(self) -> None:
        promoted_seen = set()
        for line in self.rust:
            parts = line.split("\t")
            if parts[0] != "seq":
                continue
            promoted = [int(value) for value in parts[8].split(",") if value]
            draws = int(parts[9])
            shuffle = 7 if promoted else 0
            if parts[1] == "r3":
                self.assertEqual(draws, 1 + shuffle + 12)
                self.assertTrue(all(0 <= slot <= 3 for slot in promoted))
            elif parts[1] == "r4_stage_one":
                self.assertEqual(draws, 1 + 1 + shuffle + 12)
                self.assertTrue(all(1 <= slot <= 4 for slot in promoted))
            else:
                self.assertEqual(draws, 1 + 1 + shuffle + 15)
                self.assertTrue(all(1 <= slot <= 5 for slot in promoted))
            self.assertEqual(len(promoted), len(set(promoted)))
            promoted_seen.add(bool(promoted))
        # Both branches must appear, otherwise the sweep proves only one path.
        self.assertEqual(promoted_seen, {True, False})

    def test_native_r5_seed_one_record_matches_the_fixture_bytes(self) -> None:
        record = bytes.fromhex(VECTORS["ng3_seed_1_record_hex"])
        result = generate_ng3_rarity5_effect_sequence(1, level=LEVEL)
        self.assertEqual(
            generated_effect_fields(result),
            native_effect_fields(record, len(result.effects)),
        )
        rust = self.row("r5", 1)
        self.assertEqual(rust[9], str(result.random_draws))
        self.assertEqual(int(rust[10], 16), result.final_rng_state)

    def test_native_r5_seed_241719428_record_matches_the_fixture_bytes(self) -> None:
        record = bytes.fromhex(VECTORS["ng3_seed_241719428_record_hex"])
        result = generate_ng3_rarity5_effect_sequence(241_719_428, level=LEVEL)
        self.assertEqual(
            generated_effect_fields(result),
            native_effect_fields(record, len(result.effects)),
        )

    def test_native_r3_seed_6096970_record_matches_the_fixture_bytes(self) -> None:
        record = bytes.fromhex(VECTORS["ng3_rarity3_seed_6096970_record_hex"])
        seed = struct.unpack_from("<I", record, 0x20)[0]
        level = struct.unpack_from("<H", record, 0x06)[0]
        result = generate_ng3_rarity3_effect_sequence(seed, level=level)
        self.assertEqual(
            generated_effect_fields(result),
            native_effect_fields(record, len(result.effects)),
        )
        rust = self.row("r3", seed, level)
        self.assertEqual(
            rust[8], ",".join(str(value) for value in result.promoted_source_indexes)
        )
        self.assertEqual(int(rust[10], 16), result.final_rng_state)

    def test_stage_one_is_not_the_finalized_rarity_five_record(self) -> None:
        """Keep the R4 stage-one boundary explicit while both paths agree."""

        for seed in (0, 1, 241_719_428, 183_696_634):
            stage_one = self.row("r4_stage_one", seed)
            final = self.row("r5", seed)
            self.assertEqual(stage_one[4], "E604")
            self.assertEqual(stage_one[5], "4")
            self.assertEqual(final[5], "5")
            self.assertEqual(len(stage_one[11].split(",")), 5)
            self.assertEqual(len(final[11].split(",")), 6)


if __name__ == "__main__":
    unittest.main()
