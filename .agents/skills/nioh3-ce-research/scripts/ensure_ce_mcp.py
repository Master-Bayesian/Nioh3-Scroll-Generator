from __future__ import annotations

import argparse
import hashlib
import json
import os
import subprocess
import sys
import time
from pathlib import Path
from typing import Any


ROOT = Path(__file__).resolve().parents[4]
CE_MCP_PYTHON = ROOT / ".tools" / "cheat-engine-mcp" / "python" / "src"
if CE_MCP_PYTHON.is_dir() and str(CE_MCP_PYTHON) not in sys.path:
    sys.path.insert(0, str(CE_MCP_PYTHON))

from ce_mcp_server.bridge import CheatEngineBridge


DEFAULT_CE_PATH = Path(r"C:\Program Files\Cheat Engine\cheatengine-x86_64-SSE4-AVX2.exe")
DEFAULT_LOADER_TABLE = ROOT / ".tools" / "cheat-engine-mcp" / "load_ce_mcp.CT"
DEFAULT_PLUGIN_DIR = Path(r"D:\CE\plugins\ce-mcp-v0.2.9")
EXPECTED_PLUGIN_SHA256 = "EC7B6C2F2AC38A1FCE6BCB139A1DE3D953FEDC1B035B72DAB11FF6357C935D17"
EXPECTED_CORE_SHA256 = "7B3E03452400BCDD149F0731DF4BA36911EC626CA50249414C56F4515C9C9BB6"


def sha256_file(path: Path) -> str:
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1024 * 1024), b""):
            digest.update(chunk)
    return digest.hexdigest().upper()


def verify_file(path: Path, expected_sha256: str) -> dict[str, Any]:
    if not path.is_file():
        raise FileNotFoundError(path)
    actual_sha256 = sha256_file(path)
    if actual_sha256 != expected_sha256:
        raise RuntimeError(f"Unapproved file identity: {path}: {actual_sha256} != {expected_sha256}")
    return {"path": str(path.resolve()), "size": path.stat().st_size, "sha256": actual_sha256}


def verify_artifacts(ce_path: Path, loader_table: Path, plugin_dir: Path) -> dict[str, Any]:
    if not ce_path.is_file():
        raise FileNotFoundError(ce_path)
    if not loader_table.is_file():
        raise FileNotFoundError(loader_table)
    plugin = verify_file(plugin_dir / "ce_mcp_plugin.dll", EXPECTED_PLUGIN_SHA256)
    core = verify_file(plugin_dir / "ce_mcp_plugin_core.dll", EXPECTED_CORE_SHA256)
    table_source = loader_table.read_text(encoding="utf-8")
    expected_path = str(Path(plugin["path"]))
    if "loadPlugin" not in table_source or expected_path not in table_source:
        raise RuntimeError(f"Loader table does not load the approved plugin path: {loader_table}")
    return {
        "cheat_engine": {"path": str(ce_path.resolve()), "size": ce_path.stat().st_size},
        "loader_table": {
            "path": str(loader_table.resolve()),
            "size": loader_table.stat().st_size,
            "sha256": sha256_file(loader_table),
        },
        "plugin": plugin,
        "core": core,
    }


def wait_for_session(bridge: CheatEngineBridge, timeout_seconds: float) -> list[dict[str, Any]]:
    deadline = time.monotonic() + timeout_seconds
    while time.monotonic() < deadline:
        sessions = bridge.list_sessions()
        if sessions:
            return sessions
        time.sleep(0.25)
    return []


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Ensure the approved CE MCP v0.2.9 bridge is reachable without attaching a game target."
    )
    parser.add_argument("--ce-path", type=Path, default=DEFAULT_CE_PATH)
    parser.add_argument("--loader-table", type=Path, default=DEFAULT_LOADER_TABLE)
    parser.add_argument("--plugin-dir", type=Path, default=DEFAULT_PLUGIN_DIR)
    parser.add_argument("--port", type=int, default=5556)
    parser.add_argument("--timeout-seconds", type=float, default=20.0)
    parser.add_argument("--validate-only", action="store_true")
    args = parser.parse_args()

    artifacts = verify_artifacts(args.ce_path, args.loader_table, args.plugin_dir)
    if args.validate_only:
        print(json.dumps({"ok": True, "validated_only": True, "artifacts": artifacts}, indent=2))
        return

    bridge = CheatEngineBridge(host="127.0.0.1", port=args.port)
    launched_process: subprocess.Popen[bytes] | None = None
    bridge.start()
    try:
        sessions = wait_for_session(bridge, 2.0)
        if not sessions:
            child_environment = os.environ.copy()
            child_environment["CE_MCP_PORT"] = str(args.port)
            launched_process = subprocess.Popen(
                [str(args.ce_path.resolve()), str(args.loader_table.resolve())],
                shell=False,
                env=child_environment,
            )
            sessions = wait_for_session(bridge, args.timeout_seconds)
        if not sessions:
            raise RuntimeError(
                "CE MCP did not connect. Check the visible Cheat Engine window for a Lua/plugin prompt, "
                "then inspect the approved loader registration."
            )
        if len(sessions) != 1:
            raise RuntimeError(
                "More than one CE MCP session connected; preserve them and select an exact session for live work: "
                + ", ".join(str(session.get("session_id")) for session in sessions)
            )
        session_id = str(sessions[0]["session_id"])
        attached = bridge.call_tool("ce.get_attached_process", {}, session_id, 30.0)
        if attached.get("ok") is not True:
            raise RuntimeError(f"Bridge connected but target query failed: {json.dumps(attached)}")
        print(json.dumps({
            "ok": True,
            "launched": launched_process is not None,
            "launched_ce_process_id": launched_process.pid if launched_process is not None else None,
            "session_id": session_id,
            "ce_process_id": sessions[0].get("ce_process_id"),
            "attached_target": {
                "attached": attached.get("attached"),
                "process_id": attached.get("process_id"),
                "process_name": attached.get("process_name"),
                "image_path": attached.get("image_path"),
            },
            "artifacts": artifacts,
        }, indent=2))
    finally:
        bridge.stop()


if __name__ == "__main__":
    try:
        main()
    except Exception as exc:
        print(f"ERROR {type(exc).__name__}: {exc}", file=sys.stderr, flush=True)
        raise
