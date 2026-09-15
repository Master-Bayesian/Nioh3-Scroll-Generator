"""Cross-language parity gate for the M2.2 NG3 rarity-4 finalizer slice.

`crates/nioh3-data/examples/r4_finalizer_vectors.rs` loads the shipped resource
through the real product adapter (`nioh3_data::load_effect_resource`), then
materializes the stage-one insertion record and the finalized preview through
the domain record/rarity-pair API. Every emitted row is reproduced here from
the retained reference (`nioh3_scroll_editor.effect_sequence` and
`nioh3_scroll_editor.r4_finalizer_engine`).

The tracked native corpus (`test_fixtures/r4_native_corpus`) is the oracle:
each pair is compared byte for byte against the `.bin` files, not against a
Python-derived expectation. The Python tests that already read the same files
(`tests/test_r4_finalizer_engine.py`) are retained unchanged.

Provenance: the pairs come from the live parity validator
`research/validate_ng3_rarity4_native_parity_live.py` (signature-gated native
stage generation and completion finalization in isolated remote buffers; it
never reads or writes a save) and were archived by
`research/build_r4_finalizer_corpus.py` (schema
`nioh3-r4-native-finalizer-corpus/v1`, game version 2.00.02, playthrough 3,
rarity 4). The tracked copies are sanitized relative to the private capture in
exactly eight origin-account bytes, offsets `0x02..=0x05` and `0x14..=0x17`;
those are outside the effect area and are carried through verbatim, so no
byte-exact comparison in this gate depends on them.
"""
from __future__ import annotations

import os
from pathlib import Path
import struct
import subprocess
import unittest

from nioh3_scroll_editor.effect_sequence import (
    materialize_ng3_rarity4_final_record,
    materialize_ng3_rarity4_stage_one_record,
)
from nioh3_scroll_editor.r4_finalizer_engine import R4FinalizerEngine


ROOT = Path(__file__).resolve().parents[2]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
CORPUS_ROOT = ROOT / "test_fixtures" / "r4_native_corpus"
CORPUS_DIRS = ("base", "distributed")

RECORD_SIZE = 0xE8
EFFECT_START = 0x34
EFFECT_STRIDE = 0x18
EFFECT_SLOT_COUNT = 7

LEVEL = 180
SWEEP_RECOMMENDED_LEVEL = 183
SWEEP_GENERATION_SERIAL = 3_283_000
SWEEP_TRANSFER_COUNT = 0
SWEEP_MULTIPLIER = 2_654_435_761
# Later duplicates are dropped while the emit order is kept, matching the
# emitter's `dedupe`: the stride sequence starts at 0, which repeats anchor 0.
SWEEP_SEEDS = list(
    dict.fromkeys(
        [
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
            0x7FFF_FFFF,
            0x8000_0000,
            0xFFFF_FFFE,
            0xFFFF_FFFF,
        ]
        + [(index * SWEEP_MULTIPLIER) & 0xFFFF_FFFF for index in range(48)]
    )
)
LEVEL_SWEEP = [1, 30, 90, 150, 180, 300, 500, 700, 65_535]
# Stride index 1 is deliberate: seed `SWEEP_MULTIPLIER` is level-sensitive in
# the completion path, so the level block is not vacuous for that branch.
LEVEL_SWEEP_SEEDS = list(
    dict.fromkeys(
        [
            0,
            1,
            0x7FFF_FFFF,
            0xFFFF_FFFF,
            SWEEP_MULTIPLIER,
            (5 * SWEEP_MULTIPLIER) & 0xFFFF_FFFF,
            (97 * SWEEP_MULTIPLIER) & 0xFFFF_FFFF,
        ]
    )
)
# Seeds spanning both auxiliary-mode branches and both completion outcomes.
REVEAL_SEEDS = [1, 2_965, 2_654_435_762, 2_802_362_287, 3_189_639_204, 774_553_835]

# Byte offsets of the recovered effect-slot resolved value (`i32` at +0x08).
RESOLVED_VALUE_OFFSETS = (0x08, 0x09, 0x0A, 0x0B)
# The only bytes outside the effect area that may follow the level.
LEVEL_FIELD_OFFSETS = {0x06, 0x07, 0x08, 0x09}
EFFECT_AREA_END = EFFECT_START + EFFECT_SLOT_COUNT * EFFECT_STRIDE

# The tracked corpus is sanitized against the private capture in these eight
# origin-account bytes; both records carry the same value, so the pair delta is
# unaffected and no finalizer input depends on them.
SANITIZED_OFFSETS = (0x02, 0x03, 0x04, 0x05, 0x14, 0x15, 0x16, 0x17)

REJECT_CASES = (
    "record_size_short",
    "record_size_long",
    "record_type",
    "rarity",
    "template_context",
    "promotion_target",
)
EXPECTED_REJECTIONS = {
    "record_size_short": "err:RecordLength",
    "record_size_long": "err:RecordLength",
    "record_type": "err:UnsupportedRecordType",
    "rarity": "err:UnsupportedRarity",
    "template_context": "err:TemplateRecordType",
}

# Absolute offsets of the effect-slot bytes the finalizer rewrites.
SLOT_DELTA_OFFSETS = (0x00, 0x01, 0x04, 0x05, 0x08, 0x0C, 0x0D, 0x0E)


def run_emitter() -> list[str]:
    """Run the Rust development emitter and return its tabbed output lines."""

    target = os.environ.get(
        "CARGO_TARGET_DIR", str(ROOT / ".codex_tmp" / "m22_r4_parity_target")
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
            "r4_finalizer_vectors",
            "--",
            str(DATA_ROOT),
            str(CORPUS_ROOT),
        ],
        cwd=ROOT,
        env={**os.environ, "CARGO_TARGET_DIR": target},
        capture_output=True,
        text=True,
        check=False,
    )
    if completed.returncode != 0:
        raise AssertionError(
            "Rust emitter r4_finalizer_vectors failed: "
            + (completed.stderr.strip() or completed.stdout.strip())
        )
    return [line for line in completed.stdout.splitlines() if line and "\t" in line]


def native_pairs() -> list[tuple[str, bytes, bytes]]:
    """Tracked stage/final pairs in the emitter's emit order."""

    pairs: list[tuple[str, bytes, bytes]] = []
    for directory in CORPUS_DIRS:
        for stage_path in sorted((CORPUS_ROOT / directory).glob("*_stage.bin")):
            final_path = Path(str(stage_path).replace("_stage.bin", "_final.bin"))
            pairs.append(
                (
                    directory + "/" + stage_path.name,
                    stage_path.read_bytes(),
                    final_path.read_bytes(),
                )
            )
    return pairs


def template_fields(record: bytes) -> dict[str, int]:
    return {
        "seed": struct.unpack_from("<I", record, 0x20)[0],
        "level": struct.unpack_from("<H", record, 0x06)[0],
        "recommended_level": struct.unpack_from("<H", record, 0x10)[0],
        "generation_serial": struct.unpack_from("<I", record, 0x28)[0],
        "transfer_count": struct.unpack_from("<I", record, 0xDC)[0],
    }


def attempts_text(result) -> str:
    """Mirror the emitter's one-attempt encoding field for field."""

    rendered = []
    for trace in result.attempts:
        selected = (
            "-"
            if trace.selected_effect_id is None
            else "%04X" % trace.selected_effect_id
        )
        roll = "-" if trace.roll_percent is None else str(trace.roll_percent)
        rendered.append(
            "{}:{}:{}:{}:{}:{}:{}:{}:{:08x}".format(
                trace.target_index,
                trace.assigned_category,
                trace.weight_slot,
                trace.pool_size,
                trace.total_weight,
                selected,
                roll,
                int(trace.accepted),
                trace.final_rng_state,
            )
        )
    return ";".join(rendered)


def accepted_text(accepted) -> str:
    return "-" if accepted is None else str(accepted)


def reference_pair(template: bytes, fields: dict[str, int], engine: R4FinalizerEngine):
    """Materialize one pair through the retained reference implementation."""

    stage, _sequence = materialize_ng3_rarity4_stage_one_record(template, **fields)
    final, _final_sequence = materialize_ng3_rarity4_final_record(template, **fields)
    completion = engine.finalize_completion(stage)
    if completion.record != final:
        raise AssertionError(
            "reference final-record materializer disagrees with the engine"
        )
    return stage, final, completion


def reference_lines() -> list[str]:
    engine = R4FinalizerEngine()
    pairs = native_pairs()
    donor = pairs[0][1]
    lines: list[str] = []

    for relative, stage_bytes, final_bytes in pairs:
        fields = template_fields(stage_bytes)
        stage, final, completion = reference_pair(stage_bytes, fields, engine)
        lines.append(
            "pair\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}".format(
                relative,
                fields["seed"],
                fields["level"],
                fields["recommended_level"],
                fields["generation_serial"],
                fields["transfer_count"],
                int(stage == stage_bytes),
                int(final == final_bytes),
                stage.hex(),
                final.hex(),
                accepted_text(completion.accepted_index),
                attempts_text(completion),
            )
        )

    for seed in SWEEP_SEEDS:
        fields = {
            "seed": seed,
            "level": LEVEL,
            "recommended_level": SWEEP_RECOMMENDED_LEVEL,
            "generation_serial": SWEEP_GENERATION_SERIAL,
            "transfer_count": SWEEP_TRANSFER_COUNT,
        }
        stage, final, completion = reference_pair(donor, fields, engine)
        lines.append(
            "sweep\t{}\t{}\t{}\t{}\t{}\t{}".format(
                seed,
                LEVEL,
                stage.hex(),
                final.hex(),
                accepted_text(completion.accepted_index),
                attempts_text(completion),
            )
        )

    for level in LEVEL_SWEEP:
        for seed in LEVEL_SWEEP_SEEDS:
            fields = {
                "seed": seed,
                "level": level,
                "recommended_level": SWEEP_RECOMMENDED_LEVEL,
                "generation_serial": SWEEP_GENERATION_SERIAL,
                "transfer_count": SWEEP_TRANSFER_COUNT,
            }
            stage, final, completion = reference_pair(donor, fields, engine)
            lines.append(
                "level\t{}\t{}\t{}\t{}\t{}\t{}".format(
                    seed,
                    level,
                    stage.hex(),
                    final.hex(),
                    accepted_text(completion.accepted_index),
                    attempts_text(completion),
                )
            )

    for seed in REVEAL_SEEDS:
        fields = {
            "seed": seed,
            "level": LEVEL,
            "recommended_level": SWEEP_RECOMMENDED_LEVEL,
            "generation_serial": SWEEP_GENERATION_SERIAL,
            "transfer_count": SWEEP_TRANSFER_COUNT,
        }
        stage, _sequence = materialize_ng3_rarity4_stage_one_record(donor, **fields)
        for reveal in (True, False):
            completion = engine.finalize_completion(stage, reveal=reveal)
            lines.append(
                "reveal\t{}\t{}\t{}\t{}\t{}\t{}\t{}".format(
                    seed,
                    LEVEL,
                    int(reveal),
                    stage.hex(),
                    completion.record.hex(),
                    accepted_text(completion.accepted_index),
                    attempts_text(completion),
                )
            )

    return lines


def slot_of(offset: int) -> int:
    """Effect-slot ordinal owning an absolute record offset, or 0 if none."""

    span = EFFECT_SLOT_COUNT * EFFECT_STRIDE
    if not EFFECT_START <= offset < EFFECT_START + span:
        return 0
    return (offset - EFFECT_START) // EFFECT_STRIDE + 1


def row_stage(row: list[str]) -> str:
    """Stage-record hex of a parsed emit row, whatever its kind."""

    if row[0] == "pair":
        return row[9]
    if row[0] == "reveal":
        return row[4]
    return row[3]


def row_final(row: list[str]) -> str:
    """Final-record hex of a parsed emit row, whatever its kind."""

    if row[0] == "pair":
        return row[10]
    if row[0] == "reveal":
        return row[5]
    return row[4]


def row_accepted(row: list[str]) -> str:
    """Accepted-index text of a parsed emit row, whatever its kind."""

    if row[0] == "pair":
        return row[11]
    if row[0] == "reveal":
        return row[6]
    return row[5]


def row_attempts(row: list[str]) -> str:
    """Attempt text of a parsed emit row, whatever its kind."""

    if row[0] == "pair":
        return row[12]
    if row[0] == "reveal":
        return row[7]
    return row[6]


def weight_slots(row: list[str]) -> set[int]:
    """Candidate weight slots used by a parsed emit row's attempts."""

    return {int(attempt.split(":")[2]) for attempt in row_attempts(row).split(";")}


def effect_area(record_hex: str) -> bytes:
    """Effect-slot bytes only, excluding every header/lineage field."""

    return bytes.fromhex(record_hex)[EFFECT_START:EFFECT_AREA_END]


class R4FinalizerParityTests(unittest.TestCase):
    """Production-adapter finalizer parity over the native corpus and sweeps."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.rust = run_emitter()
        cls.reference = reference_lines()
        cls.pairs = native_pairs()
        cls.rust_pairs: dict[str, list[str]] = {}
        cls.rust_sweep: dict[tuple[int, int], list[str]] = {}
        cls.rust_level: dict[tuple[int, int], list[str]] = {}
        cls.rust_reveal: dict[tuple[int, int], list[str]] = {}
        cls.rust_reject: dict[str, str] = {}
        cls.reference_pairs: dict[str, list[str]] = {}
        cls.reference_sweep: dict[tuple[int, int], list[str]] = {}
        cls.reference_level: dict[tuple[int, int], list[str]] = {}
        cls.reference_reveal: dict[tuple[int, int], list[str]] = {}
        for line in cls.rust:
            parts = line.split("\t")
            if parts[0] == "pair":
                cls.rust_pairs[parts[1]] = parts
            elif parts[0] == "sweep":
                cls.rust_sweep[(int(parts[1]), int(parts[2]))] = parts
            elif parts[0] == "level":
                cls.rust_level[(int(parts[1]), int(parts[2]))] = parts
            elif parts[0] == "reveal":
                cls.rust_reveal[(int(parts[1]), int(parts[3]))] = parts
            elif parts[0] == "reject":
                cls.rust_reject[parts[1]] = parts[2]
        for line in cls.reference:
            parts = line.split("\t")
            if parts[0] == "pair":
                cls.reference_pairs[parts[1]] = parts
            elif parts[0] == "sweep":
                cls.reference_sweep[(int(parts[1]), int(parts[2]))] = parts
            elif parts[0] == "level":
                cls.reference_level[(int(parts[1]), int(parts[2]))] = parts
            elif parts[0] == "reveal":
                cls.reference_reveal[(int(parts[1]), int(parts[3]))] = parts

    def test_emitter_covers_every_native_pair(self) -> None:
        expected = [
            directory + "/" + path.name
            for directory in CORPUS_DIRS
            for path in sorted((CORPUS_ROOT / directory).glob("*_stage.bin"))
        ]
        self.assertEqual(len(self.pairs), 10)
        self.assertEqual(sorted(self.rust_pairs), sorted(expected))

    def test_native_corpus_pairs_are_byte_exact(self) -> None:
        """The tracked `.bin` files are the oracle, not a Python expectation."""

        compared = 0
        for relative, stage_bytes, final_bytes in self.pairs:
            row = self.rust_pairs[relative]
            self.assertEqual(len(stage_bytes), RECORD_SIZE, relative)
            self.assertEqual(len(final_bytes), RECORD_SIZE, relative)
            self.assertEqual(row[7], "1", relative + ": Rust stage != native stage")
            self.assertEqual(row[8], "1", relative + ": Rust final != native final")
            self.assertEqual(bytes.fromhex(row_stage(row)), stage_bytes, relative)
            self.assertEqual(bytes.fromhex(row_final(row)), final_bytes, relative)
            compared += len(stage_bytes) + len(final_bytes)
        self.assertEqual(compared, 10 * 2 * RECORD_SIZE)

    def test_native_pair_rows_match_the_retained_reference(self) -> None:
        self.assertEqual(len(self.reference_pairs), 10)
        for relative, reference in self.reference_pairs.items():
            self.assertEqual(reference, self.rust_pairs.get(relative), relative)

    def test_pair_delta_is_confined_to_one_effect_slot(self) -> None:
        """Independent pairing audit over the tracked bytes, not the generator."""

        observed_slots: set[int] = set()
        unchanged: list[str] = []
        for relative, stage_bytes, final_bytes in self.pairs:
            deltas = [
                index
                for index in range(RECORD_SIZE)
                if stage_bytes[index] != final_bytes[index]
            ]
            slots = {slot_of(index) for index in deltas}
            self.assertLessEqual(len(slots), 1, relative + ": delta spans " + str(sorted(slots)))
            self.assertNotIn(0, slots, relative + ": delta outside the effect area")
            for index in deltas:
                slot = slot_of(index)
                observed_slots.add(slot)
                base = EFFECT_START + (slot - 1) * EFFECT_STRIDE
                self.assertIn(index - base, SLOT_DELTA_OFFSETS, relative)
            if not deltas:
                unchanged.append(relative)
        self.assertEqual(sorted(observed_slots), [2, 3, 4, 5])
        self.assertEqual(
            unchanged,
            [
                "base/sample_02_seed_2965_stage.bin",
                "distributed/sample_05_seed_2802362287_stage.bin",
            ],
        )

    def test_sanitized_origin_account_bytes_are_outside_the_effect_area(self) -> None:
        """The eight sanitized bytes round-trip and never enter a slot."""

        for relative, stage_bytes, final_bytes in self.pairs:
            rust_stage = bytes.fromhex(row_stage(self.rust_pairs[relative]))
            for offset in SANITIZED_OFFSETS:
                self.assertEqual(slot_of(offset), 0, relative + ": sanitized byte in a slot")
                self.assertEqual(stage_bytes[offset], final_bytes[offset], relative)
                self.assertEqual(rust_stage[offset], stage_bytes[offset], relative)

    def test_unchanged_bytes_are_preserved(self) -> None:
        for relative, stage_bytes, final_bytes in self.pairs:
            row = self.rust_pairs[relative]
            self.assertEqual(bytes.fromhex(row_stage(row)), stage_bytes, relative)
            self.assertEqual(bytes.fromhex(row_final(row)), final_bytes, relative)
            if row_accepted(row) == "-":
                self.assertEqual(stage_bytes, final_bytes, relative + ": no-change pair moved")
                continue
            base = EFFECT_START + int(row_accepted(row)) * EFFECT_STRIDE
            for index in range(RECORD_SIZE):
                if base <= index < base + EFFECT_STRIDE:
                    continue
                self.assertEqual(
                    stage_bytes[index],
                    final_bytes[index],
                    relative + ": byte %#04x moved" % index,
                )

    def test_second_emitter_run_reproduces_every_row(self) -> None:
        """No in-place pair mutation or order dependence leaks between cases.

        The pair API hands back two independently owned records; a second full
        emitter run must therefore reproduce all 131 rows exactly, and every
        install record must still equal its tracked native stage file.
        """

        self.assertEqual(run_emitter(), self.rust)
        for relative, stage_bytes, _final_bytes in self.pairs:
            self.assertEqual(
                bytes.fromhex(row_stage(self.rust_pairs[relative])), stage_bytes, relative
            )

    def test_sweep_rows_match_the_reference(self) -> None:
        for seed in SWEEP_SEEDS:
            self.assertIn((seed, LEVEL), self.rust_sweep, "missing sweep seed " + str(seed))
            self.assertEqual(
                self.reference_sweep[(seed, LEVEL)],
                self.rust_sweep[(seed, LEVEL)],
                "sweep seed " + str(seed),
            )
        self.assertEqual(len(self.rust_sweep), len(SWEEP_SEEDS))
        expected = (
            10
            + len(SWEEP_SEEDS)
            + len(LEVEL_SWEEP) * len(LEVEL_SWEEP_SEEDS)
            + 2 * len(REVEAL_SEEDS)
            + len(REJECT_CASES)
        )
        self.assertEqual(len(self.rust), expected)

    def test_level_sweep_rows_match_the_reference(self) -> None:
        for level in LEVEL_SWEEP:
            for seed in LEVEL_SWEEP_SEEDS:
                self.assertIn(
                    (seed, level), self.rust_level, "missing level %d seed %d" % (level, seed)
                )
                self.assertEqual(
                    self.reference_level[(seed, level)],
                    self.rust_level[(seed, level)],
                    "level %d seed %d" % (level, seed),
                )
        self.assertEqual(
            len(self.rust_level), len(LEVEL_SWEEP) * len(LEVEL_SWEEP_SEEDS)
        )

    def test_level_movement_is_measured_on_the_effect_area_not_the_level_field(self) -> None:
        """Separate real effect movement from the level header bytes.

        The corpus is level 180 only, so nothing here claims native level
        coverage. Counting whole records would be vacuous because `+0x06`/`+0x08`
        always follow the requested level; the counts below are taken over the
        effect area (`0x34..0xDC`) and over the per-slot resolved values
        (`i32` at `+0x08`) only. Measured on the shipped pc_v2_00_02 tables:
        two of the seven swept seeds move, each through seven distinct resolved
        tuples, and only `SWEEP_MULTIPLIER` also moves the finalized preview.
        """

        stage_areas: dict[int, int] = {}
        final_areas: dict[int, int] = {}
        resolved: dict[int, int] = {}
        accepted_variants: dict[int, int] = {}
        for seed in LEVEL_SWEEP_SEEDS:
            stage_blocks = {
                effect_area(row_stage(self.rust_level[(seed, level)]))
                for level in LEVEL_SWEEP
            }
            final_blocks = {
                effect_area(row_final(self.rust_level[(seed, level)]))
                for level in LEVEL_SWEEP
            }
            resolved_tuples = {
                tuple(
                    int.from_bytes(
                        bytes.fromhex(row_stage(self.rust_level[(seed, level)]))[
                            EFFECT_START + index * EFFECT_STRIDE + 0x08 :
                            EFFECT_START + index * EFFECT_STRIDE + 0x0C
                        ],
                        "little",
                    )
                    for index in range(EFFECT_SLOT_COUNT)
                )
                for level in LEVEL_SWEEP
            }
            stage_areas[seed] = len(stage_blocks)
            final_areas[seed] = len(final_blocks)
            resolved[seed] = len(resolved_tuples)
            accepted_variants[seed] = len(
                {row_accepted(self.rust_level[(seed, level)]) for level in LEVEL_SWEEP}
            )
        self.assertEqual(sorted(stage_areas.values()), [1, 1, 1, 1, 1, 7, 7])
        self.assertEqual(
            [seed for seed, value in stage_areas.items() if value > 1],
            [0x7FFF_FFFF, SWEEP_MULTIPLIER],
        )
        self.assertEqual(sorted(resolved.values()), [1, 1, 1, 1, 1, 7, 7])
        # The finalized preview keeps a level-flat effect area for
        # `0x7FFF_FFFF` because the finalizer overwrites the moving slot.
        self.assertEqual(sorted(final_areas.values()), [1, 1, 1, 1, 1, 1, 7])
        self.assertEqual(
            [seed for seed, value in final_areas.items() if value > 1],
            [SWEEP_MULTIPLIER],
        )
        self.assertEqual(set(accepted_variants.values()), {1})
        # The requested level is reported verbatim in the stage record, and no
        # byte outside the effect area except the level words may move.
        for seed in LEVEL_SWEEP_SEEDS:
            baseline = bytes.fromhex(row_stage(self.rust_level[(seed, 1)]))
            for level in LEVEL_SWEEP:
                stage = bytes.fromhex(row_stage(self.rust_level[(seed, level)]))
                self.assertEqual(struct.unpack_from("<H", stage, 0x06)[0], level)
                outside = {
                    index
                    for index in range(RECORD_SIZE)
                    if not EFFECT_START <= index < EFFECT_AREA_END
                    and stage[index] != baseline[index]
                }
                self.assertLessEqual(
                    outside, LEVEL_FIELD_OFFSETS, "%d at level %d moved %s" % (seed, level, outside)
                )
                inside = {
                    index
                    for index in range(EFFECT_START, EFFECT_AREA_END)
                    if stage[index] != baseline[index]
                }
                for index in inside:
                    self.assertIn(
                        (index - EFFECT_START) % EFFECT_STRIDE,
                        RESOLVED_VALUE_OFFSETS,
                        "%d at level %d moved a non-resolved slot byte %#04x" % (seed, level, index),
                    )

    def test_completion_decisions_are_level_sensitive_only_through_prior_rows(self) -> None:
        """The finalizer is not blindly level-invariant; report what moves.

        The finalizer RNG seed is derived from slot ids and rolls, so it is
        level-independent, and the accepted slot is identical at every swept
        level. Prior-row eligibility, however, reads the slot's resolved value
        (`value != 1` in `wrapper_prior_effect_eligible`), so a level change can
        add or drop a generated prior row and move later pool sizes. Measured
        here: exactly `SWEEP_MULTIPLIER` changes its attempt traces, and no
        swept seed changes its accepted slot.
        """

        decision_variants: dict[int, int] = {}
        for seed in LEVEL_SWEEP_SEEDS:
            decision_variants[seed] = len(
                {
                    (
                        row_accepted(self.rust_level[(seed, level)]),
                        row_attempts(self.rust_level[(seed, level)]),
                    )
                    for level in LEVEL_SWEEP
                }
            )
        self.assertEqual(sorted(decision_variants.values()), [1, 1, 1, 1, 1, 1, 2])
        self.assertEqual(
            [seed for seed, value in decision_variants.items() if value > 1],
            [SWEEP_MULTIPLIER],
        )

    def test_reveal_branch_rows_match_the_reference(self) -> None:
        self.assertEqual(len(self.rust_reveal), 2 * len(REVEAL_SEEDS))
        for key, reference in self.reference_reveal.items():
            self.assertEqual(reference, self.rust_reveal.get(key), str(key))

    def test_reveal_branch_is_reachable_and_measured(self) -> None:
        """`reveal = false` is a real, reachable branch, not dead code.

        The two flags share one stage-one record and resolve adjacent weight
        slots (`0x3C`/`0x3B` without the auxiliary-mode reveal flag, `0x3E`/
        `0x3D` with it). On the six swept seeds the flag changes the accepted
        slot for three of them and leaves the other three byte-identical.
        """

        changed: list[int] = []
        slot_deltas: set[int] = set()
        for seed in REVEAL_SEEDS:
            enabled = self.rust_reveal[(seed, 1)]
            disabled = self.rust_reveal[(seed, 0)]
            # One stage-one record feeds both branches.
            self.assertEqual(row_stage(enabled), row_stage(disabled), str(seed))
            slots_enabled = weight_slots(enabled)
            slots_disabled = weight_slots(disabled)
            self.assertEqual(len(slots_enabled), 1, str(seed))
            self.assertEqual(len(slots_disabled), 1, str(seed))
            # `reveal` raises the resolved weight slot by exactly one.
            self.assertEqual(
                next(iter(slots_enabled)) - next(iter(slots_disabled)), 1, str(seed)
            )
            slot_deltas |= slots_enabled | slots_disabled
            if row_final(enabled) != row_final(disabled):
                changed.append(seed)
        # Both auxiliary-mode branches appear, so neither slot pair is assumed.
        self.assertLessEqual({0x3B, 0x3C, 0x3D, 0x3E}, slot_deltas)
        self.assertEqual(changed, [1, 3_189_639_204, 774_553_835])

    def test_selection_and_acceptance_branches_are_nonvacuous(self) -> None:
        accepted: set = set()
        weight_slots: set[int] = set()
        attempts_seen = 0
        rows = (
            list(self.rust_pairs.values())
            + list(self.rust_sweep.values())
            + list(self.rust_level.values())
            + list(self.rust_reveal.values())
        )
        for row in rows:
            accepted_text_value = row_accepted(row)
            accepted.add(None if accepted_text_value == "-" else int(accepted_text_value))
            attempts = row_attempts(row)
            self.assertTrue(attempts, "every row must carry at least one attempt")
            for attempt in attempts.split(";"):
                weight_slots.add(int(attempt.split(":")[2]))
                attempts_seen += 1
        self.assertIn(None, accepted)
        self.assertGreaterEqual(len(accepted), 2)
        # 0x3C is the ordinary weight slot; 0x3E is the revealed branch.
        self.assertIn(0x3C, weight_slots)
        self.assertIn(0x3E, weight_slots)
        self.assertGreater(attempts_seen, len(self.rust_pairs))

    def test_rejected_inputs_fail_closed(self) -> None:
        self.assertEqual(sorted(self.rust_reject), sorted(REJECT_CASES))
        for case, expected in EXPECTED_REJECTIONS.items():
            self.assertEqual(self.rust_reject[case], expected, case)
        # The out-of-range completion target must be rejected; the exact variant
        # name belongs to the domain, so only rejection is pinned here.
        self.assertTrue(self.rust_reject["promotion_target"].startswith("err:"))


if __name__ == "__main__":
    unittest.main()
