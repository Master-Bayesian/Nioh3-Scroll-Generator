"""Cross-language parity gate for the M2.3a offline NG3 preview slice.

`crates/nioh3-data/examples/preview_vectors.rs` loads the shipped resources
through the real production adapters (`load_preview_resources` and
`load_effect_resource`) and composes the complete offline NG3 preview. Every
emitted row is reproduced here from the retained Python reference modules and
compared exactly, so a table, ordering, or composition change on either side
fails the gate instead of passing silently.

The reference side uses the modules the shipped product uses:
`auxiliary_generation.generate_complete_auxiliary` (terrain, auxiliary enemy
groups, special rules), `enemy_state_search.generate_enemy_state_preview`
(per-variant occurrences and Possessed/Curse state),
`models.ScrollCandidate.from_effect_sequence` (payload effect packing) and
`scroll_input_metadata.initial_challenge_capacity`. The Python auxiliary groups
come from the class generators, while the Rust side derives them from the
roster stage, so agreement is an independent cross-check of both ports.
"""
from __future__ import annotations

import os
from pathlib import Path
import struct
import subprocess
import unittest

from nioh3_scroll_editor.auxiliary_generation import generate_complete_auxiliary
from nioh3_scroll_editor.effect_sequence import (
    generate_ng3_rarity3_effect_sequence,
    generate_ng3_rarity4_stage_one_effect_sequence,
    generate_ng3_rarity5_effect_sequence,
)
from nioh3_scroll_editor.enemy_state_search import generate_enemy_state_preview
from nioh3_scroll_editor.grace_map import load_grace_output_map
from nioh3_scroll_editor.models import ScrollCandidate
from nioh3_scroll_editor.scroll_input_metadata import initial_challenge_capacity


ROOT = Path(__file__).resolve().parents[2]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"

SWEEP_SEEDS = 96
SWEEP_MULTIPLIER = 2_654_435_761
SEED_SWEEP = [
    0,
    1,
    2,
    2_965,
    240_348_265,
    6_096_970,
    74_063_692,
    82_212_268,
    183_696_634,
    241_719_428,
    0x0FFF_FFFF,
    0x7FFF_FFFF,
    0x8000_0000,
    0xFFFF_FFFE,
    0xFFFF_FFFF,
] + [(index * SWEEP_MULTIPLIER) & 0xFFFF_FFFF for index in range(SWEEP_SEEDS)]

EFFECT_SEED_SWEEP = [0, 1, 2, 82_212_268, 183_696_634, 241_719_428, 0xFFFF_FFFF] + [
    (index * SWEEP_MULTIPLIER) & 0xFFFF_FFFF for index in range(16)
]

PLAYTHROUGH_SWEEP = (1, 2, 3, 4, 5)
NG3_PLAYTHROUGH = 3
LEVEL = 180
VARIANTS = ("solo", "expedition")


def f32_bits(value: float) -> str:
    return f"{struct.unpack('<I', struct.pack('<f', float(value)))[0]:08X}"


def f64_bits(value: float) -> str:
    return f"{struct.unpack('<Q', struct.pack('<d', float(value)))[0]:016X}"


def optional_f32_bits(value) -> str:
    return "-" if value is None else f32_bits(value)


def optional_f64_bits(value) -> str:
    return "-" if value is None else f64_bits(value)


def text_or_dash(value) -> str:
    return "-" if value is None else str(value)


def number_or_dash(value) -> str:
    return "-" if value is None else str(value)


def flag_text(value: bool) -> str:
    return "true" if value else "false"


def auxiliary_lines(seed: int, playthrough: int) -> list[str]:
    """Component, group, entry and rule rows for one Seed/progression."""

    auxiliary = generate_complete_auxiliary(seed, playthrough)
    mode = auxiliary.mode
    terrain = auxiliary.terrain
    descriptor = auxiliary.descriptor
    rules = auxiliary.special_rules
    display_keys = ",".join(f"{key:04X}" for key in terrain.display_effect_keys)
    rule_keys = ",".join(f"{key:04X}" for key in rules.keys)
    flags = ",".join("1" if value else "0" for value in descriptor.flags)
    components = [
        f"{mode.value:02X}",
        str(mode.branch_class),
        str(terrain.value),
        str(terrain.selected_row_index),
        flag_text(terrain.used_filtered_pool),
        str(terrain.scoped_seed),
        str(descriptor.selector),
        flags,
        display_keys,
        rule_keys,
        str(rules.target_budget),
        str(rules.random_draws),
        str(rules.scoped_seed),
    ]
    lines = [f"component\t{seed}\t{playthrough}\t" + "\t".join(components)]
    for wave, group in enumerate(auxiliary.enemies.groups):
        lines.append(
            f"group\t{seed}\t{playthrough}\t{wave}\t{f32_bits(group.source_budget)}"
            f"\t{len(group.entries)}"
        )
        for position, entry in enumerate(group.entries):
            lines.append(
                f"entry\t{seed}\t{playthrough}\t{wave}\t{position}\t{entry.row_index}"
                f"\t{entry.lookup_key:08X}\t{entry.role}\t{entry.scratch_rule_key:04X}"
            )
    for slot, entry in enumerate(rules.entries):
        lines.append(
            f"rule\t{seed}\t{playthrough}\t{slot}\t{entry.key:04X}\t{entry.row_index}"
            f"\t{optional_f32_bits(entry.raw_value)}"
            f"\t{optional_f64_bits(entry.display_value)}"
            f"\t{text_or_dash(entry.display_unit)}"
            f"\t{text_or_dash(entry.display_grade)}"
            f"\t{number_or_dash(entry.value_source_offset)}"
            f"\t{text_or_dash(entry.qualifier_kind)}"
            f"\t{number_or_dash(entry.qualifier_key)}"
        )
    return lines


def state_lines(seed: int) -> list[str]:
    """Occurrence and summary rows for both NG3 mission variants."""

    lines: list[str] = []
    for variant in VARIANTS:
        preview = generate_enemy_state_preview(seed, NG3_PLAYTHROUGH, variant=variant)
        for occurrence in preview.occurrences:
            lines.append(
                f"state\t{seed}\t{variant}\t{occurrence.wave_index}"
                f"\t{occurrence.position}\t{occurrence.lookup_key}\t{occurrence.role}"
                f"\t{occurrence.source_row_index}\t{occurrence.availability}"
                f"\t{occurrence.native_spawn_key}\t{occurrence.possessed}"
                f"\t{occurrence.curse_if_fresh_null_source_selector_runs}"
            )
        lines.append(
            f"statesummary\t{seed}\t{variant}\t{preview.terrain}"
            f"\t{len(preview.occurrences)}"
            f"\t{flag_text(preview.possessed_complete)}"
            f"\t{len(preview.missing_inputs)}"
            f"\t{' | '.join(preview.missing_inputs)}"
            f"\t{preview.curse_scope}"
        )
    return lines


def effect_lines(seed: int, grace_map) -> list[str]:
    """Payload effect rows for the three certified NG3 rarity paths."""

    stage_one_map = load_grace_output_map(rarity=4)
    paths = (
        ("r3", generate_ng3_rarity3_effect_sequence(seed, level=LEVEL)),
        (
            "r4_stage_one",
            generate_ng3_rarity4_stage_one_effect_sequence(
                seed, level=LEVEL, special_mapping=stage_one_map
            ),
        ),
        (
            "r5",
            generate_ng3_rarity5_effect_sequence(
                seed, level=LEVEL, grace_mapping=grace_map
            ),
        ),
    )
    lines: list[str] = []
    for name, result in paths:
        candidate = ScrollCandidate.from_effect_sequence(result)
        for effect in candidate.effects:
            roll = "-" if effect.roll_percent is None else str(effect.roll_percent)
            lines.append(
                f"effect\t{name}\t{seed}\t{effect.slot}\t{effect.effect_id}"
                f"\t{effect.value}\t{effect.metadata}\t{effect.prefix}"
                f"\t{effect.tail_0}\t{effect.tail_1}\t{roll}"
            )
    return lines


def reference_lines() -> list[str]:
    grace_map = load_grace_output_map(rarity=5)
    lines: list[str] = []
    for seed in SEED_SWEEP:
        for playthrough in PLAYTHROUGH_SWEEP:
            lines.extend(auxiliary_lines(seed, playthrough))
            if playthrough == NG3_PLAYTHROUGH:
                lines.append(f"capacity\t{seed}\t{initial_challenge_capacity(seed)}")
                lines.extend(state_lines(seed))
    for seed in EFFECT_SEED_SWEEP:
        lines.extend(effect_lines(seed, grace_map))
    return lines


def run_example(*arguments: str) -> list[str]:
    """Run the Rust development emitter and return its tabbed output lines."""

    target = os.environ.get(
        "CARGO_TARGET_DIR", str(ROOT / ".codex_tmp" / "m23_preview_parity_target")
    )
    completed = subprocess.run(
        [
            "cargo",
            "run",
            "--locked",
            "--offline",
            "--quiet",
            "--manifest-path",
            str(ROOT / "crates" / "nioh3-data" / "Cargo.toml"),
            "--example",
            "preview_vectors",
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
            "Rust preview emitter failed: "
            + (completed.stderr.strip() or completed.stdout.strip())
        )
    return [line for line in completed.stdout.splitlines() if line and "\t" in line]


class PreviewParityTest(unittest.TestCase):
    """Field-exact comparison against the retained Python reference."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.rust = run_example(str(DATA_ROOT))
        cls.reference = reference_lines()

    def test_row_counts_match(self) -> None:
        self.assertEqual(len(self.rust), len(self.reference))

    def test_rows_match_exactly(self) -> None:
        for index, (rust_line, reference_line) in enumerate(
            zip(self.rust, self.reference)
        ):
            self.assertEqual(rust_line, reference_line, f"row {index}")

    def test_sweep_is_not_vacuous(self) -> None:
        """Pin the sweep shape so a silently empty gate cannot pass."""

        kinds: dict[str, int] = {}
        for line in self.reference:
            kind = line.split("\t", 1)[0]
            kinds[kind] = kinds.get(kind, 0) + 1
        self.assertEqual(kinds["component"], len(SEED_SWEEP) * len(PLAYTHROUGH_SWEEP))
        self.assertEqual(kinds["capacity"], len(SEED_SWEEP))
        self.assertEqual(kinds["statesummary"], len(SEED_SWEEP) * len(VARIANTS))
        # The reference keeps only the produced slots: five for the rarity-3
        # and rarity-4 stage-one sequences, six for rarity 5.
        self.assertEqual(kinds["effect"], len(EFFECT_SEED_SWEEP) * (5 + 5 + 6))

        components = [line for line in self.reference if line.startswith("component\t")]
        fields = [line.split("\t") for line in components]
        self.assertEqual(
            {row[4] for row in fields}, {"0", "1", "2"}, "branch classes"
        )
        self.assertGreaterEqual(
            len({int(row[6]) for row in fields}), 15, "measured terrain rows"
        )
        display_keys = {row[11] for row in fields}
        self.assertTrue(
            any("0024" in keys for keys in display_keys),
            "the Crucible display key never occurs in the sweep",
        )
        self.assertTrue(
            any("," in keys for keys in display_keys), "no two-key terrain rows"
        )

        rules = [
            line.split("\t") for line in self.reference if line.startswith("rule\t")
        ]
        self.assertTrue(rules, "the special-rule block is empty")
        units = {row[8] for row in rules}
        self.assertTrue(
            {"percent", "seconds", "grade"}.issubset(units),
            f"missing display units in the sweep: {sorted(units)}",
        )
        grades = {row[9] for row in rules if row[9] != "-"}
        self.assertEqual(
            grades,
            {"A", "B", "C"},
            "the graded rule family must stay measured across the sweep",
        )

        states = [
            line.split("\t") for line in self.reference if line.startswith("state\t")
        ]
        self.assertTrue(states, "the enemy-state block is empty")
        self.assertEqual(
            {row[8] for row in states}, {"base", "expedition_only"}, "availability"
        )
        self.assertEqual({row[10] for row in states}, {"yes", "no"}, "Possessed")
        self.assertTrue(
            {"guaranteed", "never"}.issubset({row[11] for row in states}),
            "both conditional Curse bounds must be measured",
        )

        summaries = [
            line.split("\t")
            for line in self.reference
            if line.startswith("statesummary\t")
        ]
        self.assertTrue(
            all(row[5] == "true" for row in summaries),
            "a complete capture must report possessed_complete",
        )
        self.assertTrue(
            all(row[6] == "1" for row in summaries),
            "exactly the Curse line is missing on a complete capture",
        )

        effects = [
            line.split("\t") for line in self.reference if line.startswith("effect\t")
        ]
        self.assertEqual(
            {row[1] for row in effects}, {"r3", "r4_stage_one", "r5"}, "rarity paths"
        )
        for path, slots in (("r3", 5), ("r4_stage_one", 5), ("r5", 6)):
            per_seed = len([row for row in effects if row[1] == path]) // len(
                EFFECT_SEED_SWEEP
            )
            self.assertEqual(per_seed, slots, path)
        self.assertTrue(
            all(row[10] != "-" for row in effects),
            "an effect-sequence preview must carry roll_percent",
        )
        self.assertTrue(
            all(int(row[6]) & 0xFF == int(row[10]) for row in effects),
            "metadata must pack roll_percent in its low byte",
        )


if __name__ == "__main__":
    unittest.main()
