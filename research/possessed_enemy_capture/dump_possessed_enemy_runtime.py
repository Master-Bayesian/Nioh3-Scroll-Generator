from __future__ import annotations

import argparse
import hashlib
import json
import struct
import sys
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any

ROOT = Path(__file__).resolve().parents[2]
CE_MCP_PYTHON = ROOT / ".tools" / "cheat-engine-mcp" / "python" / "src"
if CE_MCP_PYTHON.is_dir() and str(CE_MCP_PYTHON) not in sys.path:
    sys.path.insert(0, str(CE_MCP_PYTHON))

from ce_mcp_server.bridge import CheatEngineBridge

ROW_STRIDE = 0x398
SCROLL_STRIDE = 0xE8
SCROLL_CAPACITY = 400
MAX_TABLE_ROWS = 100_000
MAX_HASH_ENTRIES = 500_000
READ_CHUNK_SIZE = 512 * 1024


def require_ok(operation: str, result: dict[str, Any]) -> dict[str, Any]:
    if result.get("ok") is not True:
        raise RuntimeError(f"{operation} failed: {json.dumps(result, ensure_ascii=False)}")
    return result


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


class RuntimeReader:
    def __init__(self, bridge: CheatEngineBridge, session_id: str) -> None:
        self.bridge = bridge
        self.session_id = session_id

    def lua(self, expression: str) -> Any:
        result = require_ok("lua metadata read", self.bridge.call_tool(
            "ce.lua_exec", {"script": f"return {expression}"}, self.session_id, 30.0
        ))
        return result.get("result", result.get("value", result))

    def read(self, address: int, size: int) -> bytes:
        if address <= 0 or size < 0:
            raise ValueError(f"Invalid read range: {address:#x} + {size:#x}")
        parts: list[bytes] = []
        for offset in range(0, size, READ_CHUNK_SIZE):
            count = min(READ_CHUNK_SIZE, size - offset)
            result = require_ok("memory read", self.bridge.call_tool(
                "ce.read_memory", {"address": address + offset, "size": count},
                self.session_id, 30.0
            ))
            chunk = bytes.fromhex(str(result.get("bytes_hex", "")))
            if len(chunk) != count:
                raise RuntimeError(f"Partial memory read at {address + offset:#x}: {len(chunk)} != {count}")
            parts.append(chunk)
        return b"".join(parts)

    def u32(self, address: int) -> int:
        return struct.unpack("<I", self.read(address, 4))[0]

    def u64(self, address: int) -> int:
        return struct.unpack("<Q", self.read(address, 8))[0]


def save_blob(directory: Path, name: str, address: int, data: bytes) -> dict[str, Any]:
    path = directory / name
    path.write_bytes(data)
    return {
        "file": name,
        "address": f"0x{address:X}",
        "size": len(data),
        "sha256": sha256(data),
    }


def capture_enemy_context(reader: RuntimeReader, output: Path, manager: int, offset: int) -> dict[str, Any]:
    context = reader.u64(manager + offset)
    if context == 0:
        return {"manager_offset": f"0x{offset:X}", "context_address": "0x0", "available": False}
    context_blob = reader.read(context, 0x80)
    row_table = struct.unpack_from("<Q", context_blob, 0)[0]
    hash_object = struct.unpack_from("<Q", context_blob, 0x20)[0]
    if row_table == 0 or hash_object == 0:
        raise RuntimeError(f"Incomplete enemy context at {context:#x}")
    row_count = reader.u32(row_table + 4)
    if row_count > MAX_TABLE_ROWS:
        raise RuntimeError(f"Enemy row count exceeds bound: {row_count}")
    row_blob = reader.read(row_table, 8 + row_count * ROW_STRIDE)
    hash_header = reader.read(hash_object, 0x20)
    hash_begin, hash_end = struct.unpack_from("<QQ", hash_header, 8)
    if hash_end < hash_begin or (hash_end - hash_begin) % 8:
        raise RuntimeError("Enemy hash range is invalid")
    hash_count = (hash_end - hash_begin) // 8
    if hash_count > MAX_HASH_ENTRIES:
        raise RuntimeError(f"Enemy hash count exceeds bound: {hash_count}")
    hash_blob = reader.read(hash_begin, (hash_count + 1) * 8) if hash_begin else b""
    prefix = f"enemy_context_{offset:02x}"
    return {
        "manager_offset": f"0x{offset:X}",
        "context_address": f"0x{context:X}",
        "row_table_address": f"0x{row_table:X}",
        "row_count": row_count,
        "row_stride": ROW_STRIDE,
        "hash_object_address": f"0x{hash_object:X}",
        "hash_begin": f"0x{hash_begin:X}",
        "hash_end": f"0x{hash_end:X}",
        "hash_entry_count": hash_count,
        "hash_entry_layout": "little-endian u32 lookup_key, u32 row_index; one trailing entry retained",
        "context": save_blob(output, f"{prefix}_context.bin", context, context_blob),
        "rows": save_blob(output, f"{prefix}_rows.bin", row_table, row_blob),
        "hash": save_blob(output, f"{prefix}_hash.bin", hash_begin, hash_blob),
    }


def main() -> None:
    parser = argparse.ArgumentParser(description="Dump read-only PC v2.01 selector databases from an existing game process.")
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--port", type=int, default=5556)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=True)

    bridge = CheatEngineBridge(host="127.0.0.1", port=args.port)
    bridge.start()
    try:
        deadline = time.monotonic() + 20.0
        session_id: str | None = None
        while time.monotonic() < deadline:
            sessions = bridge.list_sessions()
            if sessions:
                session_id = sessions[0]["session_id"]
                break
            time.sleep(0.25)
        if session_id is None:
            raise RuntimeError("Cheat Engine bridge did not connect within 20 seconds")
        require_ok("attach", bridge.call_tool(
            "ce.attach_process", {"process_id": args.pid}, session_id, 30.0
        ))
        attached = require_ok("target identity", bridge.call_tool(
            "ce.get_attached_process", {}, session_id, 30.0
        ))
        if str(attached.get("process_name", "")).casefold() != "nioh3.exe":
            raise RuntimeError(f"Refusing unexpected process: {attached.get('process_name')!r}")
        reader = RuntimeReader(bridge, session_id)
        base = int(reader.lua("assert(getAddressSafe('Nioh3.exe'))"))

        context_global = reader.u64(base + 0x474D4E0)
        data_root = reader.u64(context_global) if context_global else 0
        if data_root == 0:
            raise RuntimeError("Scroll data root is unavailable")
        scroll_container = data_root + 0x224A60
        scroll_bytes = reader.read(scroll_container, SCROLL_CAPACITY * SCROLL_STRIDE)
        scroll_metadata = reader.read(data_root + 0x16A80, 0x80)

        manager = reader.u64(base + 0x45B5DF0)
        if manager == 0:
            raise RuntimeError("Parameter manager is unavailable")
        selector = reader.read(manager + 0xB0A, 1)[0]
        manager_blob = reader.read(manager, 0xC00)
        enemy_contexts = [
            capture_enemy_context(reader, output, manager, 0x38),
            capture_enemy_context(reader, output, manager, 0x40),
        ]

        config_holder = reader.u64(base + 0x45B5E00)
        config_root = reader.u64(config_holder) if config_holder else 0
        fallback_context = reader.u64(manager + 0x230)
        manifest: dict[str, Any] = {
            "schema": "nioh3-pc-v2.01-possessed-enemy-runtime-tables/v2",
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "run_id": args.run_id,
            "read_only": True,
            "writes_game_memory": False,
            "pid": args.pid,
            "module_base": f"0x{base:X}",
            "scroll_lookup": {
                "context_global_address": f"0x{base + 0x474D4E0:X}",
                "context_address": f"0x{context_global:X}",
                "data_root_address": f"0x{data_root:X}",
                "container_address": f"0x{scroll_container:X}",
                "capacity": SCROLL_CAPACITY,
                "stride": SCROLL_STRIDE,
                "container": save_blob(output, "scroll_container_400xE8.bin", scroll_container, scroll_bytes),
                "metadata": save_blob(output, "scroll_container_metadata.bin", data_root + 0x16A80, scroll_metadata),
            },
            "enemy_weight_database": {
                "manager_global_address": f"0x{base + 0x45B5DF0:X}",
                "manager_address": f"0x{manager:X}",
                "selector_offset": "0xB0A",
                "selector": selector,
                "manager": save_blob(output, "parameter_manager.bin", manager, manager_blob),
                "contexts": enemy_contexts,
            },
            "configuration": {
                "holder_global_address": f"0x{base + 0x45B5E00:X}",
                "holder_address": f"0x{config_holder:X}",
                "root_address": f"0x{config_root:X}",
                "lookup_context_offset": "0x2820",
                "fallback_context_address": f"0x{fallback_context:X}",
            },
        }
        if config_root:
            manifest["configuration"]["root"] = save_blob(
                output, "configuration_root.bin", config_root, reader.read(config_root, 0x2A20)
            )
        if fallback_context:
            manifest["configuration"]["fallback_context"] = save_blob(
                output, "configuration_fallback_context.bin", fallback_context,
                reader.read(fallback_context, 0x400)
            )
        manifest_path = output / "runtime_tables_manifest.json"
        manifest_path.write_text(json.dumps(manifest, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps({"manifest": str(manifest_path), "run_id": args.run_id}, ensure_ascii=False))
    finally:
        bridge.stop()


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR {type(exc).__name__}: {exc}", file=sys.stderr)
        raise
