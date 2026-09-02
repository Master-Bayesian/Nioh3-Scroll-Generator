from __future__ import annotations

"""Capture isolated native auxiliary-generation vectors from profiled Nioh 3.

The probe creates a private record and descriptor inside a temporary remote
allocation. It never points the game at inventory memory and never reads or
writes save data. The game's normal descriptor constructor is called, the
pointer-bearing result is converted to a pointer-free JSON representation, and
the game's normal descriptor destructor is called before the allocation is
released.
"""

import argparse
import ctypes
import hashlib
import json
import struct
import sys
from datetime import datetime, timezone
from pathlib import Path
from typing import Sequence

from ctypes import wintypes

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.auxiliary_generation import generate_auxiliary_mode
from nioh3_scroll_editor.native import (
    NativeBatchOracle,
    WAIT_OBJECT_0,
    load_native_runtime_profile,
)


EXPECTED_GAME_VERSION = "PC v2.00.02"
RECORD_TYPE_R3 = 0xE604
RECORD_CONTEXT_BYTE = 1
DESCRIPTOR_SIZE = 0x25
OUTER_STRIDE = 0x28
INNER_STRIDE = 0x14

INIT_DESCRIPTOR_RVA = 0x1B6E658
BUILD_DESCRIPTOR_RVA = 0x20DD430
DESTROY_DESCRIPTOR_RVA = 0x1B91E80

INIT_DESCRIPTOR_SIGNATURE = bytes.fromhex(
    "40 53 48 83 EC 20 48 8B D9 33 C0 33 C9"
)
BUILD_DESCRIPTOR_SIGNATURE = bytes.fromhex(
    "48 83 EC 38 48 8B D1 0F B7 09 E8"
)
DESTROY_DESCRIPTOR_SIGNATURE = bytes.fromhex(
    "40 53 48 83 EC 20 48 8B D9 48 8B 09"
)


def _parse_int(value: str) -> int:
    return int(value, 0)


def build_auxiliary_capture_wrapper(
    *,
    record_address: int,
    descriptor_address: int,
    init_function: int,
    build_function: int,
) -> bytes:
    """Build the ABI-correct isolated constructor wrapper."""

    return b"".join(
        (
            bytes.fromhex("53 56 48 83 EC 28"),
            b"\x48\xBB" + struct.pack("<Q", record_address),
            b"\x48\xBE" + struct.pack("<Q", descriptor_address),
            bytes.fromhex("48 89 F1"),
            b"\x48\xB8" + struct.pack("<Q", init_function),
            bytes.fromhex("FF D0"),
            bytes.fromhex("48 89 D9 45 33 C0 49 89 F1"),
            b"\x48\xB8" + struct.pack("<Q", build_function),
            bytes.fromhex("FF D0 31 C0 48 83 C4 28 5E 5B C3"),
        )
    )


def build_auxiliary_destroy_wrapper(
    *,
    descriptor_address: int,
    destroy_function: int,
) -> bytes:
    """Build the ABI-correct descriptor destructor wrapper."""

    return b"".join(
        (
            bytes.fromhex("48 83 EC 28"),
            b"\x48\xB9" + struct.pack("<Q", descriptor_address),
            b"\x48\xB8" + struct.pack("<Q", destroy_function),
            bytes.fromhex("FF D0 31 C0 48 83 C4 28 C3"),
        )
    )


def _run_remote_wrapper(
    oracle: NativeBatchOracle,
    wrapper: bytes,
    *,
    timeout_ms: int,
) -> None:
    if oracle.process is None or oracle.allocation is None:
        raise RuntimeError("native oracle session is not open")
    oracle.write(oracle.allocation, wrapper)
    thread_id = wintypes.DWORD()
    thread = oracle.dll.CreateRemoteThread(
        oracle.process,
        None,
        0,
        oracle.allocation,
        None,
        0,
        ctypes.byref(thread_id),
    )
    if not thread:
        raise ctypes.WinError(ctypes.get_last_error())
    try:
        wait_result = oracle.dll.WaitForSingleObject(thread, timeout_ms)
        if wait_result != WAIT_OBJECT_0:
            raise RuntimeError(f"native auxiliary wrapper wait failed: {wait_result:#x}")
        exit_code = wintypes.DWORD()
        if not oracle.dll.GetExitCodeThread(thread, ctypes.byref(exit_code)):
            raise ctypes.WinError(ctypes.get_last_error())
        if exit_code.value != 0:
            raise RuntimeError(
                f"native auxiliary wrapper returned {exit_code.value:#x}"
            )
    finally:
        oracle.dll.CloseHandle(thread)


def _bounded_count(begin: int, end: int, stride: int, maximum: int, label: str) -> int:
    if begin == 0 and end == 0:
        return 0
    if begin == 0 or end < begin or (end - begin) % stride:
        raise RuntimeError(
            f"invalid native {label} vector: begin=0x{begin:X}, end=0x{end:X}"
        )
    count = (end - begin) // stride
    if count > maximum:
        raise RuntimeError(f"native {label} vector count is implausible: {count}")
    return count


def _describe_descriptor(oracle: NativeBatchOracle, address: int) -> dict[str, object]:
    raw = oracle.read(address, DESCRIPTOR_SIZE)
    outer_begin, outer_end, outer_capacity = struct.unpack_from("<QQQ", raw, 0)
    outer_count = _bounded_count(
        outer_begin,
        outer_end,
        OUTER_STRIDE,
        32,
        "outer",
    )
    if outer_capacity < outer_end:
        raise RuntimeError("native outer vector capacity precedes its end")

    groups: list[dict[str, object]] = []
    for group_index in range(outer_count):
        outer = oracle.read(outer_begin + group_index * OUTER_STRIDE, OUTER_STRIDE)
        inner_begin, inner_end, inner_capacity = struct.unpack_from("<QQQ", outer, 0)
        inner_count = _bounded_count(
            inner_begin,
            inner_end,
            INNER_STRIDE,
            128,
            f"inner[{group_index}]",
        )
        if inner_capacity < inner_end:
            raise RuntimeError(
                f"native inner[{group_index}] vector capacity precedes its end"
            )
        entries: list[dict[str, object]] = []
        for entry_index in range(inner_count):
            entry = oracle.read(
                inner_begin + entry_index * INNER_STRIDE,
                INNER_STRIDE,
            )
            words = struct.unpack("<4I", entry[:0x10])
            sanitized_entry = bytearray(entry)
            # Native only initializes byte +0x10. Bytes +0x11..+0x13 are
            # structure padding and retain unrelated stack data.
            sanitized_entry[0x11:0x14] = bytes(3)
            entries.append(
                {
                    "index": entry_index,
                    "sanitized_raw_hex": sanitized_entry.hex().upper(),
                    "field_00": f"0x{words[0]:08X}",
                    "lookup_key": f"0x{words[1]:08X}",
                    "field_08": f"0x{words[2]:08X}",
                    "field_0C": f"0x{words[3]:08X}",
                    "field_10_byte": entry[0x10],
                }
            )
        groups.append(
            {
                "index": group_index,
                "entries": entries,
                "field_18": f"0x{struct.unpack_from('<I', outer, 0x18)[0]:08X}",
                "field_1C_float": struct.unpack_from("<f", outer, 0x1C)[0],
                "field_20": f"0x{struct.unpack_from('<I', outer, 0x20)[0]:08X}",
                "field_24": outer[0x24],
            }
        )

    sanitized = bytearray(raw)
    sanitized[0:0x18] = bytes(0x18)
    return {
        "sanitized_descriptor_hex": sanitized.hex().upper(),
        "special_rule_keys": [
            f"0x{value:04X}" for value in struct.unpack_from("<3H", raw, 0x18)
        ],
        "auxiliary_mode": raw[0x1E],
        "terrain": raw[0x1F],
        "selector": raw[0x20],
        "flags": [bool(raw[0x21]), bool(raw[0x22]), bool(raw[0x23])],
        "constructed": bool(raw[0x24]),
        "groups": groups,
    }


def _make_record(seed: int) -> bytes:
    if not 0 <= seed <= 0xFFFFFFFF:
        raise ValueError("seed must fit in uint32")
    record = bytearray(0xE8)
    struct.pack_into("<H", record, 0, RECORD_TYPE_R3)
    record[0x0F] = RECORD_CONTEXT_BYTE
    struct.pack_into("<I", record, 0x20, seed)
    return bytes(record)


def _capture_seed(
    oracle: NativeBatchOracle,
    seed: int,
    *,
    timeout_ms: int,
    init_descriptor_rva: int = INIT_DESCRIPTOR_RVA,
    build_descriptor_rva: int = BUILD_DESCRIPTOR_RVA,
    destroy_descriptor_rva: int = DESTROY_DESCRIPTOR_RVA,
) -> dict[str, object]:
    if oracle.allocation is None:
        raise RuntimeError("native oracle session is not open")
    record_address = oracle.source_address
    descriptor_address = oracle.destination_address
    init_function = oracle.module_base + init_descriptor_rva
    build_function = oracle.module_base + build_descriptor_rva
    destroy_function = oracle.module_base + destroy_descriptor_rva

    oracle.write(record_address, _make_record(seed))
    oracle.write(descriptor_address, bytes(0xE8))
    wrapper = build_auxiliary_capture_wrapper(
        record_address=record_address,
        descriptor_address=descriptor_address,
        init_function=init_function,
        build_function=build_function,
    )
    _run_remote_wrapper(oracle, wrapper, timeout_ms=timeout_ms)
    try:
        descriptor = _describe_descriptor(oracle, descriptor_address)
    finally:
        destroy_wrapper = build_auxiliary_destroy_wrapper(
            descriptor_address=descriptor_address,
            destroy_function=destroy_function,
        )
        _run_remote_wrapper(oracle, destroy_wrapper, timeout_ms=timeout_ms)

    mode = generate_auxiliary_mode(seed)
    return {
        "seed": seed,
        "seed_hex": f"0x{seed:08X}",
        "record_type": f"0x{RECORD_TYPE_R3:04X}",
        "record_context_byte": RECORD_CONTEXT_BYTE,
        "expected_branch_class": mode.branch_class,
        "expected_auxiliary_mode": mode.value,
        "descriptor": descriptor,
    }


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Capture pointer-free native auxiliary vectors in private remote buffers"
        )
    )
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument(
        "--seed",
        action="append",
        type=_parse_int,
        dest="seeds",
        help="Seed to capture; may be supplied more than once",
    )
    parser.add_argument("--repeat", type=int, default=2)
    parser.add_argument("--timeout-ms", type=int, default=60_000)
    parser.add_argument("--runtime-profile", type=Path)
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    if args.output.exists():
        raise FileExistsError(f"refusing to overwrite output: {args.output}")
    if not 1 <= args.repeat <= 16:
        raise ValueError("repeat must be in 1..16")
    seeds = args.seeds or [203900415, 1, 1664]
    if len(set(seeds)) != len(seeds):
        raise ValueError("duplicate seeds are not allowed")

    expected_game_version = EXPECTED_GAME_VERSION
    native_profile = None
    signatures = {
        "init_descriptor": (
            INIT_DESCRIPTOR_RVA,
            INIT_DESCRIPTOR_SIGNATURE,
        ),
        "build_descriptor": (
            BUILD_DESCRIPTOR_RVA,
            BUILD_DESCRIPTOR_SIGNATURE,
        ),
        "destroy_descriptor": (
            DESTROY_DESCRIPTOR_RVA,
            DESTROY_DESCRIPTOR_SIGNATURE,
        ),
    }
    if args.runtime_profile is not None:
        payload = json.loads(args.runtime_profile.read_text(encoding="utf-8"))
        text_sites = payload.get("text_sites", {})

        def profiled_site(name: str) -> tuple[int, bytes]:
            raw = text_sites.get(name)
            if not isinstance(raw, dict) or raw.get("rva") is None:
                raise ValueError(f"runtime profile site {name} is unresolved")
            captured = raw.get("captured_signature")
            if not captured:
                raise ValueError(
                    f"runtime profile site {name} has no captured signature"
                )
            return int(str(raw["rva"]), 0), bytes.fromhex(str(captured))

        signatures = {
            name: profiled_site(name)
            for name in ("init_descriptor", "build_descriptor", "destroy_descriptor")
        }
        expected_game_version = str(payload["display_version"])
        native_profile = load_native_runtime_profile(args.runtime_profile)

    oracle_options = {"max_batch_size": 1}
    if native_profile is not None:
        oracle_options["runtime_profile"] = native_profile
    with NativeBatchOracle(**oracle_options) as oracle:
        signature_report: dict[str, object] = {}
        for name, (rva, expected) in signatures.items():
            actual = oracle.read(oracle.module_base + rva, len(expected))
            signature_report[name] = {
                "rva": f"0x{rva:X}",
                "expected": expected.hex(" ").upper(),
                "actual": actual.hex(" ").upper(),
                "matches": actual == expected,
            }
            if actual != expected:
                raise RuntimeError(
                    f"{name} signature does not match Nioh 3 {expected_game_version}"
                )

        captures = []
        for seed in seeds:
            repeats = [
                _capture_seed(
                    oracle,
                    seed,
                    timeout_ms=args.timeout_ms,
                    init_descriptor_rva=signatures["init_descriptor"][0],
                    build_descriptor_rva=signatures["build_descriptor"][0],
                    destroy_descriptor_rva=signatures["destroy_descriptor"][0],
                )
                for _ in range(args.repeat)
            ]
            semantic_payloads = [
                json.dumps(item["descriptor"], sort_keys=True) for item in repeats
            ]
            captures.append(
                {
                    "seed": seed,
                    "all_repeats_identical": len(set(semantic_payloads)) == 1,
                    "repeats": repeats,
                }
            )

    report = {
        "schema": "nioh3-native-auxiliary-vector-capture/v1",
        "expected_game_version": expected_game_version,
        "captured_at_utc": datetime.now(timezone.utc).isoformat(),
        "code_signatures": signature_report,
        "captures": captures,
        "safety": {
            "writes_to_private_remote_allocation": True,
            "writes_to_inventory": False,
            "reads_or_writes_save": False,
            "creates_remote_threads": True,
            "calls_native_descriptor_destructor": True,
        },
    }
    payload = (json.dumps(report, ensure_ascii=False, indent=2) + "\n").encode(
        "utf-8"
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(payload)
    print(
        json.dumps(
            {
                "output": str(args.output.resolve()),
                "sha256": hashlib.sha256(payload).hexdigest().upper(),
                "seed_count": len(seeds),
                "repeat_count": args.repeat,
                "all_repeats_identical": all(
                    item["all_repeats_identical"] for item in captures
                ),
                "writes_to_inventory": False,
                "writes_to_save": False,
            },
            ensure_ascii=False,
            indent=2,
        )
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
