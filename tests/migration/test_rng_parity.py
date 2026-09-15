"""Compare the Rust migration slice with the retained product implementation.

Run through tools/run_python_tests.ps1. Rust is required: a missing toolchain
is a failed migration gate, not a skipped parity claim.
"""

from itertools import zip_longest
import os
from pathlib import Path
import shutil
import struct
import subprocess

import pytest

from nioh3_scroll_editor.enemy_state_rng import (
    LcgStream, MT19937, cvtt_i32, lottery_10000, native_shuffle, state_after,
    threshold_from_config,
)
from tests.migration.cargo_target import resolved_cargo_target_dir


ROOT = Path(__file__).resolve().parents[2]
SEEDS = (0, 1, 5489, 86872488, 156062997, 0xFFFFFFFF)
JUMPS = (0, 1, 2, 35, 623, 624, 625, 1248, 1300, 1 << 32, (1 << 64) - 1)
BOUNDS = (0, 1, 9, 65535, 2147483648, 4294967294, 4294967295)
CVTT_BITS = (
    0x0000000000000000, 0x8000000000000000, 0x3FF8000000000000,
    0xBFF8000000000000, 0x41DFFFFFFFC00000, 0x41DFFFFFFFFFFFFF,
    0x41E0000000000000, 0xC1E0000000000000, 0xC1E0000000200000,
    0x7FF0000000000000, 0xFFF0000000000000, 0x7FF8000000000000,
)
THRESHOLD_BASES = (-2147483648, -16777217, -1, 0, 1, 16777217, 2147483647)
MULTIPLIER_BITS = (
    0x00000000, 0x80000000, 0x3F800000, 0x3F7FFFFF, 0x3F800001,
    0x3F000000, 0xBF800000, 0x7F800000, 0xFF800000, 0x7FC00000,
    0x00000001, 0x7F7FFFFF,
)


@pytest.fixture(scope="module")
def rust_vectors():
    cargo = shutil.which("cargo")
    assert cargo, "Install Rust to run the required cross-language migration gate"
    env = dict(os.environ)
    env.setdefault("CARGO_TARGET_DIR", resolved_cargo_target_dir("v080-domain"))
    result = subprocess.run(
        [cargo, "run", "--release", "--quiet", "--locked", "--offline", "--manifest-path",
         str(ROOT / "crates/nioh3-domain/Cargo.toml"), "--example", "rng_vectors"],
        cwd=ROOT, env=env, text=True, encoding="utf-8", capture_output=True,
        timeout=180,
    )
    assert result.returncode == 0, result.stderr
    groups = {}
    for line in result.stdout.splitlines():
        kind, *fields = line.split("\t")
        groups.setdefault(kind, []).append(fields)
    assert set(groups) == {
        "lottery", "lcg", "jump", "mt", "bounded", "shuffle", "cvtt", "threshold",
    }
    return groups


def assert_rows(actual, expected):
    for index, (got, want) in enumerate(zip_longest(actual, expected)):
        assert got == want, f"vector {index}: Rust={got!r}, Python={want!r}"


def test_every_lottery_input(rust_vectors):
    # Exhaustive uint16 domain, including 29039 where integer division differs.
    assert_rows(rust_vectors["lottery"], (
        [str(high), str(lottery_10000(high))] for high in range(65536)
    ))


def test_float_conversion_and_threshold_boundaries(rust_vectors):
    assert_rows(rust_vectors["cvtt"], (
        [str(bits), str(cvtt_i32(struct.unpack("<d", struct.pack("<Q", bits))[0]))]
        for bits in CVTT_BITS
    ))

    def thresholds():
        for base in THRESHOLD_BASES:
            for bits in MULTIPLIER_BITS:
                row = bytearray(32)
                struct.pack_into("<i", row, 0x10, base)
                struct.pack_into("<I", row, 0x18, bits)
                yield list(map(str, (base, bits, threshold_from_config(bytes(row)))))

    assert_rows(rust_vectors["threshold"], thresholds())


def test_lcg_states_draw_counts_and_large_jumps(rust_vectors):
    def sequential():
        for seed in SEEDS:
            stream = LcgStream(seed)
            for _ in range(1300):
                high = stream.u16("migration-parity")
                yield list(map(str, (seed, stream.draws, stream.state, high)))

    assert_rows(rust_vectors["lcg"], sequential())
    assert_rows(rust_vectors["jump"], (
        list(map(str, (seed, draw, state_after(seed, draw))))
        for seed in SEEDS for draw in JUMPS
    ))


def test_mt_outputs_across_multiple_refills(rust_vectors):
    def expected():
        for seed in SEEDS:
            stream = MT19937(seed)
            for draw in range(1, 1301):
                yield list(map(str, (seed, draw, stream.u32())))

    assert_rows(rust_vectors["mt"], expected())


def test_bounded_sampling_values_and_consumption(rust_vectors):
    def expected():
        rejections = 0
        for seed in SEEDS:
            for upper in BOUNDS:
                stream = MT19937(seed)
                for step in range(1, 65):
                    value = stream.inclusive(upper)
                    yield list(map(str, (
                        seed, upper, step, value, stream.draws, stream.rejections,
                    )))
                rejections += stream.rejections
        assert rejections > 0, "The corpus must exercise rejection sampling"

    assert_rows(rust_vectors["bounded"], expected())


def test_forward_shuffle_order_and_consumption(rust_vectors):
    def expected():
        for seed in SEEDS:
            for length in (0, 1, 2, 10, 625):
                stream = MT19937(seed)
                values = list(range(length))
                native_shuffle(values, stream)
                yield [str(seed), str(length), ",".join(map(str, values)),
                       str(stream.draws), str(stream.rejections)]

    assert_rows(rust_vectors["shuffle"], expected())
