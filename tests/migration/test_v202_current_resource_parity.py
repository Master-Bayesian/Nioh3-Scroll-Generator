"""Same-v202 Rust/Python rarity-4 parity gate for the current resource version.

Offline only.  Acceptance for this ticket is three things:

1. The Rust materializer and the Python reference select the SAME offline
   resource for PC v2.02, proven by a ``resource`` identity row that the Rust
   emitter prints from the rows its loader actually handed over.
2. The two named worker seeds ``226061463`` / ``10030700`` produce byte-identical
   Rust and Python stage-one plus paired final records under that version.
3. The legacy PC v2.00.02 path is unchanged: an argument-free emitter run still
   reproduces the shipped behaviour row for row.

The Rust byte emitter is ``crates/nioh3-data/examples/r4_finalizer_vectors.rs``;
the Python reference is ``materialize_ng3_rarity4_stage_one_record`` /
``materialize_ng3_rarity4_final_record`` with
``effect_generation_tables_for_game_version((2, 0, 2, 0))``.
"""
from __future__ import annotations

import hashlib
import os
import struct
import subprocess
import unittest
from pathlib import Path

from nioh3_scroll_editor.effect_generation_tables import (
    effect_generation_tables_for_game_version,
    load_default_effect_generation_tables,
)
from nioh3_scroll_editor.effect_sequence import (
    materialize_ng3_rarity4_final_record,
    materialize_ng3_rarity4_stage_one_record,
)
from nioh3_scroll_editor.r4_finalizer_engine import R4FinalizerEngine
from nioh3_scroll_editor.r4_finalizer_resource import (
    load_default_r4_finalizer_resource,
    resource_root_for_game_version,
)
from tests.migration.cargo_target import resolved_cargo_target_dir


ROOT = Path(__file__).resolve().parents[2]
DATA_ROOT = ROOT / "nioh3_scroll_editor" / "data"
CORPUS_ROOT = ROOT / "test_fixtures" / "r4_native_corpus"
CORPUS_DIRS = ("base", "distributed")

# The offline generation resource version this release ships with, and the
# legacy shipped version whose payload PC v2.01 aliases.
V202 = (2, 0, 2, 0)
V202_FLAG = "2.0.2.0"
LEGACY_FLAG = "2.0.0.2"
LEGACY_R4_DIR = "r4_finalizer/pc_v2_00_02/resource_v1"
V202_R4_DIR = "r4_finalizer/pc_v2_02/resource_v1"

# The two worker seeds named by the recovery ticket.
NAMED_SEEDS = (226_061_463, 10_030_700)

LEVEL = 180
RECOMMENDED_LEVEL = 183
GENERATION_SERIAL = 3_283_000
TRANSFER_COUNT = 0

RECORD_SIZE = 0xE8
EFFECT_START = 0x34
EFFECT_STRIDE = 0x18
EFFECT_SLOT_COUNT = 7
EFFECT_AREA_END = EFFECT_START + EFFECT_SLOT_COUNT * EFFECT_STRIDE

# Native table blobs carry a four-byte tag plus a little-endian row count.
TABLE_HEADER_BYTES = 8

# Record fields the two implementations patch, for first-difference messages.
NAMED_FIELDS = (
    (0x00, 2, "record_type"),
    (0x06, 2, "level"),
    (0x08, 2, "level_echo"),
    (0x0C, 2, "completion_salt"),
    (0x10, 2, "recommended_level"),
    (0x12, 2, "recommended_level_echo"),
    (0x20, 4, "seed"),
    (0x28, 4, "generation_serial"),
    (0x30, 1, "rarity"),
    (0x31, 1, "rarity_echo"),
    (0x33, 1, "challenge_attempts"),
    (0xDC, 4, "transfer_count"),
)


def field_name(offset: int) -> str:
    """Human name for one absolute record offset, for mismatch reporting."""

    for start, width, name in NAMED_FIELDS:
        if start <= offset < start + width:
            return f"{name} +0x{offset - start:02X} (field 0x{start:02X})"
    if EFFECT_START <= offset < EFFECT_AREA_END:
        slot = (offset - EFFECT_START) // EFFECT_STRIDE + 1
        within = (offset - EFFECT_START) % EFFECT_STRIDE
        return f"effect slot {slot} +0x{within:02X}"
    return f"unmapped byte 0x{offset:04X}"


def first_difference(left: bytes, right: bytes) -> str | None:
    """The first differing byte, named, or ``None`` when the records are equal."""

    if left == right:
        return None
    limit = min(len(left), len(right))
    for offset in range(limit):
        if left[offset] != right[offset]:
            return (
                f"{field_name(offset)}: left=0x{left[offset]:02X} "
                f"right=0x{right[offset]:02X}"
            )
    return f"record length: left={len(left)} right={len(right)}"


def run_emitter(*extra: str) -> str:
    """Run the Rust emitter and return its stdout."""

    target = resolved_cargo_target_dir("v202-res")
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
            *extra,
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
    return completed.stdout


def rows_of(output: str, kind: str) -> list[list[str]]:
    return [line.split("\t") for line in output.splitlines() if line.startswith(kind + "\t")]


def pair_rows(output: str) -> dict[str, list[str]]:
    return {row[1]: row for row in rows_of(output, "pair")}


def resource_row(output: str) -> list[str]:
    rows = rows_of(output, "resource")
    if len(rows) != 1:
        raise AssertionError(f"expected exactly one resource row, got {len(rows)}")
    return rows[0]


def version_seed_rows(output: str) -> dict[int, list[str]]:
    return {int(row[1]): row for row in rows_of(output, "version-seed")}


def table_payload(root: Path, name: str) -> bytes:
    """Header-stripped rows of one table blob, as both loaders expose them."""

    raw = (root / "tables" / f"{name}.bin").read_bytes()
    return raw[TABLE_HEADER_BYTES:]


def table_digest(root: Path, name: str) -> tuple[int, str]:
    """``(row count, uppercase sha256)`` of one table's header-stripped rows."""

    raw = (root / "tables" / f"{name}.bin").read_bytes()
    count = struct.unpack_from("<I", raw, 4)[0]
    return (count, hashlib.sha256(raw[TABLE_HEADER_BYTES:]).hexdigest().upper())


def template_fields(template: bytes) -> dict[str, int]:
    return {
        "seed": struct.unpack_from("<I", template, 0x20)[0],
        "level": struct.unpack_from("<H", template, 0x06)[0],
        "recommended_level": struct.unpack_from("<H", template, 0x10)[0],
        "generation_serial": struct.unpack_from("<I", template, 0x28)[0],
        "transfer_count": struct.unpack_from("<I", template, 0xDC)[0],
    }


def reference_pair(template: bytes, fields: dict[str, int], tables) -> tuple[bytes, bytes]:
    """Materialize one paired stage/final record through the Python reference."""

    stage, _sequence = materialize_ng3_rarity4_stage_one_record(
        template, **fields, tables=tables
    )
    final, _final_sequence = materialize_ng3_rarity4_final_record(
        template, **fields, tables=tables
    )
    completion = R4FinalizerEngine(tables=tables).finalize_completion(stage)
    if completion.record != final:
        raise AssertionError("the final-record materializer disagrees with the engine")
    return stage, final


def corpus_donor() -> bytes:
    """The first tracked stage template, the same donor the emitter uses."""

    first = sorted((CORPUS_ROOT / CORPUS_DIRS[0]).glob("*_stage.bin"))[0]
    return first.read_bytes()


def named_seed_fields(seed: int) -> dict[str, int]:
    """The lineage the emitter applies to an explicit ``--seeds`` request."""

    return {
        "seed": seed,
        "level": LEVEL,
        "recommended_level": RECOMMENDED_LEVEL,
        "generation_serial": GENERATION_SERIAL,
        "transfer_count": TRANSFER_COUNT,
    }


class FirstDifferenceReportingTests(unittest.TestCase):
    """The mismatch reporter must name a field, not just report inequality."""

    def test_named_header_field_is_reported(self) -> None:
        left = bytearray(RECORD_SIZE)
        right = bytearray(RECORD_SIZE)
        right[0x28] = 1
        report = first_difference(bytes(left), bytes(right))
        self.assertIsNotNone(report)
        self.assertIn("generation_serial", report)

    def test_effect_slot_field_is_reported(self) -> None:
        left = bytearray(RECORD_SIZE)
        right = bytearray(RECORD_SIZE)
        right[EFFECT_START + EFFECT_STRIDE + 0x02] = 0x5A
        report = first_difference(bytes(left), bytes(right))
        self.assertIsNotNone(report)
        self.assertIn("effect slot 2 +0x02", report)

    def test_equal_records_report_nothing(self) -> None:
        record = bytes(RECORD_SIZE)
        self.assertIsNone(first_difference(record, record))


class V202CurrentResourceParityTests(unittest.TestCase):
    """Same-resource-version rarity-4 parity between Rust and Python."""

    @classmethod
    def setUpClass(cls) -> None:
        cls.shipped_root: Path = load_default_r4_finalizer_resource().root
        cls.v202_root: Path = resource_root_for_game_version(V202)
        cls.v202_tables = effect_generation_tables_for_game_version(V202)
        cls.shipped_tables = load_default_effect_generation_tables()
        cls.v202_output = run_emitter(
            "--resource-version",
            V202_FLAG,
            "--seeds",
            ",".join(str(seed) for seed in NAMED_SEEDS),
        )
        cls.legacy_output = run_emitter("--resource-version", LEGACY_FLAG)
        cls.default_output = run_emitter()

    def test_python_resolves_the_versioned_v202_resource(self) -> None:
        self.assertNotEqual(self.shipped_root, self.v202_root)
        self.assertEqual(self.v202_root.name, "resource_v1")
        self.assertEqual(self.v202_root.parent.name, "pc_v2_02")
        self.assertTrue((self.v202_root / "manifest.json").is_file())
        self.assertEqual(self.v202_root, DATA_ROOT / V202_R4_DIR)

    def test_rust_selects_the_v202_resource_for_the_same_version(self) -> None:
        """The Rust loader's own identity row must be the PC v2.02 tables.

        The row is printed from the rows the loader handed over, so it proves
        selection instead of restating source text.
        """

        row = resource_row(self.v202_output)
        self.assertEqual(row[1], V202_FLAG)
        self.assertEqual(row[2], V202_R4_DIR)
        for index, name in ((3, "item"), (5, "optional_multiplier")):
            count, digest = table_digest(self.v202_root, name)
            self.assertEqual(int(row[index]), count, f"{name} row count")
            self.assertEqual(row[index + 1], digest, f"{name} rows")

    def test_legacy_version_flag_reproduces_the_shipped_resource(self) -> None:
        """PC v2.00.02 aliases the shipped payload, and the flag must match it."""

        row = resource_row(self.legacy_output)
        self.assertEqual(row[1], LEGACY_FLAG)
        self.assertEqual(row[2], LEGACY_R4_DIR)
        for index, name in ((3, "item"), (5, "optional_multiplier")):
            count, digest = table_digest(self.shipped_root, name)
            self.assertEqual(int(row[index]), count, f"{name} row count")
            self.assertEqual(row[index + 1], digest, f"{name} rows")
        # The two resources genuinely differ, so the match above is not vacuous.
        self.assertNotEqual(
            table_digest(self.shipped_root, "optional_multiplier"),
            table_digest(self.v202_root, "optional_multiplier"),
        )

    def test_named_seeds_match_rust_v202_bytes(self) -> None:
        """Rust stage-one plus paired final bytes equal Python under PC v2.02."""

        emitted = version_seed_rows(self.v202_output)
        self.assertEqual(sorted(emitted), sorted(NAMED_SEEDS))
        # Non-vacuity: two different seeds must not collapse to one record.
        self.assertNotEqual(emitted[NAMED_SEEDS[0]][3], emitted[NAMED_SEEDS[1]][3])
        donor = corpus_donor()
        for seed in NAMED_SEEDS:
            row = emitted[seed]
            self.assertEqual(int(row[2]), LEVEL)
            stage, final = reference_pair(donor, named_seed_fields(seed), self.v202_tables)
            rust_stage = bytes.fromhex(row[3])
            rust_final = bytes.fromhex(row[4])
            self.assertEqual(len(rust_stage), RECORD_SIZE, f"seed {seed} stage length")
            self.assertEqual(len(rust_final), RECORD_SIZE, f"seed {seed} final length")
            stage_delta = first_difference(rust_stage, stage)
            final_delta = first_difference(rust_final, final)
            self.assertIsNone(
                stage_delta, f"seed {seed}: Rust vs Python v2.02 stage: {stage_delta}"
            )
            self.assertIsNone(
                final_delta, f"seed {seed}: Rust vs Python v2.02 final: {final_delta}"
            )

    def test_default_invocation_keeps_the_legacy_row_set(self) -> None:
        """Legacy regression: no flags still means the shipped v2.00.02 rows.

        The existing M2.2 parity gate counts this output exactly, so the new
        blocks must stay absent unless they are asked for.
        """

        self.assertEqual(rows_of(self.default_output, "resource"), [])
        self.assertEqual(rows_of(self.default_output, "version-seed"), [])
        pairs = pair_rows(self.default_output)
        self.assertEqual(len(pairs), 10)
        for relative, row in sorted(pairs.items()):
            directory, file_name = relative.split("/", 1)
            template = (CORPUS_ROOT / directory / file_name).read_bytes()
            fields = template_fields(template)
            stage, final = reference_pair(template, fields, self.shipped_tables)
            stage_delta = first_difference(bytes.fromhex(row[9]), stage)
            final_delta = first_difference(bytes.fromhex(row[10]), final)
            self.assertIsNone(
                stage_delta, f"{relative}: legacy stage regression: {stage_delta}"
            )
            self.assertIsNone(
                final_delta, f"{relative}: legacy final regression: {final_delta}"
            )

    def test_legacy_flag_matches_the_default_run_byte_for_byte(self) -> None:
        """`--resource-version legacy` is the historical loader, not a new path."""

        self.assertEqual(rows_of(self.legacy_output, "version-seed"), [])
        self.assertEqual(
            pair_rows(self.legacy_output),
            pair_rows(self.default_output),
        )


if __name__ == "__main__":
    unittest.main()
