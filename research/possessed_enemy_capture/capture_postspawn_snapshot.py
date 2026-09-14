from __future__ import annotations

import argparse
import ctypes
import json
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

from research.possessed_enemy_capture.run_possessed_enemy_observer import (
    CheatEngineBridge,
    call_session,
    result_value,
    verify_target,
    write_json,
)


PROCESS_QUERY_LIMITED_INFORMATION = 0x1000


class FileTime(ctypes.Structure):
    _fields_ = [("low", ctypes.c_uint32), ("high", ctypes.c_uint32)]


def process_started_at(pid: int) -> datetime:
    kernel32 = ctypes.WinDLL("kernel32", use_last_error=True)
    handle = kernel32.OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, False, pid)
    if not handle:
        raise OSError(ctypes.get_last_error(), f"OpenProcess failed for PID {pid}")
    try:
        creation = FileTime()
        exit_time = FileTime()
        kernel = FileTime()
        user = FileTime()
        if not kernel32.GetProcessTimes(
            handle,
            ctypes.byref(creation),
            ctypes.byref(exit_time),
            ctypes.byref(kernel),
            ctypes.byref(user),
        ):
            raise OSError(ctypes.get_last_error(), f"GetProcessTimes failed for PID {pid}")
        ticks = (creation.high << 32) | creation.low
        return datetime.fromtimestamp(ticks / 10_000_000 - 11_644_473_600, timezone.utc)
    finally:
        kernel32.CloseHandle(handle)


def parse_timestamp(value: str) -> datetime:
    parsed = datetime.fromisoformat(value.replace("Z", "+00:00"))
    if parsed.tzinfo is None:
        raise ValueError("Source capture timestamp must include a timezone")
    return parsed.astimezone(timezone.utc)


def source_contract(source: dict[str, Any], pid: int) -> tuple[int, str, datetime]:
    metadata = source.get("capture_metadata")
    bridge_result = source.get("bridge_result")
    if not isinstance(metadata, dict) or not isinstance(bridge_result, dict):
        raise ValueError("Source must be a file-backed late-mask capture")
    if int(metadata.get("requested_pid", 0)) != pid:
        raise ValueError("Source capture PID does not match the requested process")
    captured_at = parse_timestamp(str(metadata.get("captured_at_utc", metadata.get("captured_at", ""))))
    events = bridge_result.get("events")
    if not isinstance(events, list):
        raise ValueError("Source capture does not contain breakpoint events")
    owners = {
        int(str(event["owner_address"]), 16)
        for event in events
        if isinstance(event, dict)
        and event.get("site") == "late_mask_final"
        and event.get("owner_address")
    }
    if len(owners) != 1:
        raise ValueError(f"Source capture must contain exactly one final owner address, got {len(owners)}")
    module_base = str(bridge_result.get("module_base", ""))
    if not module_base.startswith("0x"):
        raise ValueError("Source capture does not contain a module base")
    return owners.pop(), module_base, captured_at


def snapshot_lua(pid: int, owner: int) -> str:
    return f"""
assert(getOpenedProcessID()=={pid},'Unexpected target PID')
local base=assert(getAddressSafe('Nioh3.exe'),'Nioh3.exe not attached')
local owner=0x{owner:X}
local function hx(v) return string.format('0x%X',v) end
local function u8(a,l) local b=readBytes(a,1,true); assert(b and #b==1,'Unreadable '..l); return b[1] end
local function u32(a,l) local v=readInteger(a); assert(type(v)=='number','Unreadable '..l); return v & 0xFFFFFFFF end
local function u64(a,l) local v=readQword(a); assert(type(v)=='number','Unreadable '..l); return v end
local function bytesHex(a,n,l)
  local b=readBytes(a,n,true); assert(b and #b==n,'Unreadable '..l)
  local o={{}}; for i=1,n do o[i]=string.format('%02X',b[i]) end; return table.concat(o)
end
local breakpoints=debug_getBreakpointList()
local debugging=debug_isDebugging()
assert((breakpoints==nil and not debugging) or
  (type(breakpoints)=='table' and next(breakpoints)==nil),
  'Existing or unknown debugger breakpoints must be preserved')
local begin=u64(owner,'record vector begin')
local count=u64(owner+8,'record vector count')
assert(begin~=0 and count>0 and count<=512,'Implausible record vector')
local records={{}}
for i=0,count-1 do
  local rec=u64(begin+i*0x10+8,'record pointer')
  assert(rec~=0,'Null record pointer')
  records[#records+1]={{
    index=i,record_address=hx(rec),record_raw_hex=bytesHex(rec,0xF0,'record'),
    spawn_id=hx(u32(rec+0x20,'spawn id')),mission_key=hx(u32(rec+0x24,'mission key')),
    enemy_lookup_key=hx(u32(rec+0x28,'enemy lookup key')),
    field_8e=u8(rec+0x8E,'record+0x8E'),field_8f=u8(rec+0x8F,'record+0x8F'),
    selector_class=u8(rec+0x90,'selector class'),assigned_index=u8(rec+0x95,'assigned index'),
    candidate=u8(rec+0xE9,'candidate flag'),field_ea=u8(rec+0xEA,'record+0xEA')
  }}
end
return {{read_only=true,writes_game_memory=false,pid=getOpenedProcessID(),module_base=hx(base),
  debugger_active=debugging,breakpoints=breakpoints,owner_address=hx(owner),vector_begin=hx(begin),
  vector_count=count,selection_mask_hex=bytesHex(owner+0x1C4,12,'selection mask'),records=records}}
"""


def record_identity(record: dict[str, Any]) -> tuple[object, ...]:
    return (
        record.get("index"),
        record.get("record_address"),
        record.get("spawn_id"),
        record.get("mission_key"),
        record.get("enemy_lookup_key"),
    )


def validate_samples(
    samples: list[dict[str, Any]],
    source_module_base: str,
    expected_candidate_count: int = 6,
    expected_spawn_id: str = "0xF40",
    expected_enemy_key: str = "0x8BC34",
    expected_candidate_identities: list[tuple[object, ...]] | None = None,
) -> dict[str, Any]:
    if len(samples) != 2:
        raise ValueError("Exactly two samples are required")
    first, second = samples
    for sample in samples:
        if sample.get("module_base") != source_module_base:
            raise RuntimeError("Module base changed from the source capture")
        if sample.get("read_only") is not True or sample.get("writes_game_memory") is not False:
            raise RuntimeError("Snapshot did not preserve the read-only contract")
    stable_header = all(
        first.get(key) == second.get(key)
        for key in ("pid", "module_base", "owner_address", "vector_begin", "vector_count")
    )
    first_records = first.get("records")
    second_records = second.get("records")
    if not isinstance(first_records, list) or not isinstance(second_records, list):
        raise RuntimeError("Snapshot records are missing")
    stable_identities = [record_identity(item) for item in first_records] == [
        record_identity(item) for item in second_records
    ]
    if not stable_header or not stable_identities:
        raise RuntimeError("Owner or record identities changed between consecutive reads")
    target_records = [
        record
        for record in second_records
        if record.get("mission_key") == "0xCC96"
        and record.get("spawn_id") == expected_spawn_id
        and record.get("enemy_lookup_key") == expected_enemy_key
    ]
    if len(target_records) != 1:
        raise RuntimeError(f"Expected exactly one target task record, got {len(target_records)}")
    mission_candidates = [
        record
        for record in second_records
        if record.get("mission_key") == "0xCC96" and record.get("candidate") == 1
    ]
    if len(mission_candidates) != expected_candidate_count:
        raise RuntimeError(
            f"Expected {expected_candidate_count} task candidates, got {len(mission_candidates)}"
        )
    source_fingerprint_matched = expected_candidate_identities is None or [
        record_identity(record) for record in mission_candidates
    ] == expected_candidate_identities
    if not source_fingerprint_matched:
        raise RuntimeError("Live task-candidate identities do not match the source capture")
    return {
        "stable_header": stable_header,
        "stable_record_identities": stable_identities,
        "raw_records_stable": [item.get("record_raw_hex") for item in first_records]
        == [item.get("record_raw_hex") for item in second_records],
        "source_candidate_fingerprint_matched": source_fingerprint_matched,
        "target_record": target_records[0],
        "mission_candidates": mission_candidates,
    }


def main() -> int:
    parser = argparse.ArgumentParser(description="Capture a bounded read-only post-spawn candidate snapshot.")
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--session-id", required=True)
    parser.add_argument("--port", type=int, default=5556)
    parser.add_argument("--source-capture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--expected-candidate-count", type=int, default=6)
    parser.add_argument("--expected-spawn-id", default="0xF40")
    parser.add_argument("--expected-enemy-key", default="0x8BC34")
    parser.add_argument(
        "--owner-observation",
        default="The project owner reported that the target state was visible before collection.",
    )
    args = parser.parse_args()

    output = args.output.resolve()
    if output.exists():
        raise FileExistsError(f"Refusing to overwrite existing evidence: {output}")
    source_path = args.source_capture.resolve()
    source = json.loads(source_path.read_text(encoding="utf-8"))
    owner, source_module_base, source_captured_at = source_contract(source, args.pid)
    final_event = next(
        event
        for event in source["bridge_result"]["events"]
        if isinstance(event, dict) and event.get("site") == "late_mask_final"
    )
    expected_candidate_identities = [
        record_identity(record)
        for record in final_event["records"]
        if record.get("mission_key") == "0xCC96" and record.get("candidate") == 1
    ]
    process_start = process_started_at(args.pid)
    if process_start >= source_captured_at:
        raise RuntimeError("Current PID started after the source capture; refusing possible PID reuse")

    bridge = CheatEngineBridge(host="127.0.0.1", port=args.port)
    bridge.start()
    try:
        deadline = time.monotonic() + 20.0
        while time.monotonic() < deadline:
            if any(session.get("session_id") == args.session_id for session in bridge.list_sessions()):
                break
            time.sleep(0.25)
        else:
            raise RuntimeError(f"Requested Cheat Engine session is not connected: {args.session_id}")
        attached = call_session(
            bridge, args.session_id, "target identity", "ce.get_attached_process", {}, 30.0
        )
        target = verify_target(attached, args.pid)
        samples = []
        for index in range(2):
            raw = call_session(
                bridge,
                args.session_id,
                f"post-spawn snapshot {index + 1}",
                "ce.lua_exec",
                {"script": snapshot_lua(args.pid, owner)},
                60.0,
            )
            value = result_value(raw)
            if not isinstance(value, dict):
                raise RuntimeError("Cheat Engine returned a non-object snapshot")
            samples.append(value)
            if index == 0:
                time.sleep(0.25)
        validation = validate_samples(
            samples,
            source_module_base,
            expected_candidate_count=args.expected_candidate_count,
            expected_spawn_id=args.expected_spawn_id,
            expected_enemy_key=args.expected_enemy_key,
            expected_candidate_identities=expected_candidate_identities,
        )
        write_json(
            output,
            {
                "capture_metadata": {
                    "captured_at_utc": datetime.now(timezone.utc).isoformat(),
                    "classification": "read-only post-spawn snapshot; not a pre-spawn breakpoint repeat",
                    "requested_pid": args.pid,
                    "process_started_at_utc": process_start.isoformat(),
                    "session_id": args.session_id,
                    "port": args.port,
                    "source_capture": str(source_path),
                    "source_captured_at_utc": source_captured_at.isoformat(),
                    "owner_observation": args.owner_observation,
                    "expected_candidate_count": args.expected_candidate_count,
                    "expected_spawn_id": args.expected_spawn_id,
                    "expected_enemy_key": args.expected_enemy_key,
                },
                "target": target,
                "validation": validation,
                "samples": samples,
            },
        )
    finally:
        bridge.stop()
    print(json.dumps({"output": str(output), "validation": validation}, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
