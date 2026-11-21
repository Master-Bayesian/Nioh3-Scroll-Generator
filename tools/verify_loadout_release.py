"""Bind a packaged optimizer's complete scene to its compiled MAIN and host E2E.

`seal` records build inputs, checks address/layout consistency and PE exports. It
is an identity/preflight record, NOT proof of numerical correctness. `check`
inspects a directory, ZIP, or the exact final one-file EXE. A release additionally
requires an external host receipt bound to that final EXE (no circular hash).
All checks use explicit exceptions and still run under `python -O`.
"""
from __future__ import annotations

import argparse
import hashlib
import io
import json
import re
import struct
import zipfile
from pathlib import Path, PurePosixPath

CONTRACT = "runtime-contract.json"
SCENE_ROOT = "worker/runtime/data/loadout/"
SCHEMA = "nioh3-loadout-runtime-contract/v1"
HOST_SCHEMA = "nioh3-packaged-loadout-e2e/v1"
FOOTER = struct.Struct("<16sQ32s")
REQUIRED = {
    "bundle.json", "arena.bin", "event_plan.json", "frame.bin", "gpu_setup.txt",
    "hits.txt", "live_groups.txt", "main_cpu.dll", "main_device.dll", "mapped.bin",
    "programs.txt", "source_numeric.bin", "survival_groups.txt", "tables.bin",
}
ABIS = {"nk_context_abi": 2, "nk_main_cpu_components_abi": 1, "nk_main_device_components_abi": 1}
EXPORTS = {
    "main_cpu.dll": {"nk_context_abi", "nk_main_cpu_open", "nk_main_cpu_run", "nk_main_cpu_close", "nk_main_cpu_components_abi", "nk_main_cpu_run_components"},
    "main_device.dll": {"nk_context_abi", "nk_main_device_open", "nk_main_device_close", "nk_main_device_components_abi", "nk_main_device_run_components"},
}


def need(condition: bool, message: str) -> None:
    if not condition:
        raise ValueError(message)


def digest(raw: bytes) -> str:
    return hashlib.sha256(raw).hexdigest()


def identity(raw: bytes) -> dict:
    return {"bytes": len(raw), "sha256": digest(raw)}


def safe_name(name: str) -> bool:
    path = PurePosixPath(name)
    return bool(name) and not path.is_absolute() and "\\" not in name and ":" not in name and all(part not in ("", ".", "..") for part in name.split("/"))


def inventory(files: dict[str, bytes]) -> dict:
    return {name: identity(raw) for name, raw in sorted(files.items()) if name != CONTRACT}


def public_frame(raw: bytes, plan: dict) -> bytes:
    """Keep the byte layout and typed attack policies; all player numeric buffers start empty."""
    frame = sections(raw)
    layout = {name: bytes(len(frame[name])) for name in ("stat", "d0", "s_base", "d_base")}
    need(len(frame["addresses"]) == 72, "unexpected frame address layout")
    layout["addresses"] = frame["addresses"][:56] + bytes(16)
    for name, width in (("packet_kind", 2), ("aad_packet_kind", 1)):
        if name in frame:
            need(len(frame[name]) == width, "invalid packet selector")
            layout[name] = frame[name]
    need(not {"packet_kind", "aad_packet_kind"} <= layout.keys(), "conflicting packet selectors")
    for name in ("held_reaper", "weapon_element_packet"):
        if name in frame:
            need(frame[name] == bytes([1]), "invalid attack policy: " + name)
            layout[name] = frame[name]
    need(("held_reaper" in layout) == (plan["mode"]["kind"] == "held_reaper"), "held action layout mismatch")
    packets = [row["packet"] for row in plan["templates"]]
    need(bool(packets), "action has no templates")
    if "hit_packets" in frame or any(row["kind"] != "prepared_actor" for row in packets):
        # Recreate only typed public words; never copy raw projectile object,
        # owner, or incarnation fields from an actor's historical event plan.
        records = []
        for packet in packets:
            if packet["kind"] == "prepared_actor":
                records.append(bytes(12))
            elif packet["kind"] == "unimbued_owner_projectile":
                records.append(struct.pack("<III", 1, packet["power"], packet["main_flags"]))
            else:
                raise ValueError("unsupported event packet policy: " + packet["kind"])
        expected = b"".join(records)
        need(frame.get("hit_packets") == expected, "per-contact metadata differs from the event plan")
        layout["hit_packets"] = expected
    if "identity_ki_seed" in frame:
        need(frame["identity_ki_seed"] == struct.pack("<4I", 1, 0, 0, 0x3f800000), "unknown Ki layout")
        layout["identity_ki_seed"] = frame["identity_ki_seed"]
    return b"".join(
        struct.pack("<I", len(name)) + name.encode("ascii") + struct.pack("<I", len(value)) + value
        for name, value in layout.items())


def export_layout(source: Path, target: Path) -> None:
    """Export the single direct-contact layout consumed by Engine::open_with_context.

    Frame::same_layout checks canonical addresses, packet mode and Ki seed mode.
    The character's encoded context supplies the actual STAT/D and source program.
    """
    files = {name: (source / name).read_bytes() for name in REQUIRED}
    plan = json.loads(files["event_plan.json"])
    templates = plan.get("templates", [])
    need(plan.get("mode") == {"kind": "fixed_contacts"} and len(templates) == 1
         and templates[0].get("packet") == {"kind": "prepared_actor"}
         and plan.get("single_use") is None, "layout export requires the single direct actor contact")
    frame = sections(files["frame.bin"])
    # These sections change Frame::same_layout. Extend publication of other
    # attack shapes only alongside matching exporter and engine controls.
    need(not (frame.keys() & {"packet_kind", "aad_packet_kind", "weapon_element_packet", "hit_packets", "held_reaper"}),
         "layout export requires the default weapon packet")
    files["frame.bin"] = public_frame(files["frame.bin"], plan)
    # Only slot ranges come from this base setup. The app supplies the owned
    # pool separately. Reject a nonempty static pool instead of dropping it.
    setup = files["gpu_setup.txt"].decode("ascii").splitlines()
    need(len(setup) == 3 and setup[0].split()[-1:] == ["0"] and setup[1].split() == ["0"],
         "layout export requires the app-owned equipment pool")
    files["gpu_setup.txt"] = b"0 0 0\n0\n0 0 0 0 0 0 0\n"
    target.mkdir(parents=True, exist_ok=False)
    for name, raw in files.items():
        (target / name).write_bytes(raw)


def sections(raw: bytes) -> dict[str, bytes]:
    result = {}
    offset = 0
    while offset < len(raw):
        need(offset + 4 <= len(raw), "truncated frame section name length")
        length = struct.unpack_from("<I", raw, offset)[0]
        offset += 4
        need(0 < length <= 128 and offset + length + 4 <= len(raw), "invalid frame section name")
        name = raw[offset:offset + length].decode("ascii")
        offset += length
        size = struct.unpack_from("<I", raw, offset)[0]
        offset += 4
        need(offset + size <= len(raw) and name not in result, "truncated or duplicate frame section")
        result[name] = raw[offset:offset + size]
        offset += size
    return result


def native_layout(source: bytes) -> dict:
    text = source.decode("utf-8-sig")
    constants = {}
    for name in ("MAIN_MODULE", "MAIN_ENTRY_RSP", "MAIN_SCORE"):
        match = re.search(r"^#define\s+" + name + r"\s+(0x[0-9a-fA-F]+)(?:ull|ULL)?\s*$", text, re.M)
        need(match is not None, "generated MAIN lacks " + name)
        constants[name] = int(match.group(1), 16)
    match = re.search(r"MAIN_DOMAIN\[\]\[2\]\s*=\s*\{(.*?)\};", text, re.S)
    need(match is not None, "generated MAIN domain missing")
    spans = [[int(a, 16), int(b, 16)] for a, b in re.findall(r"\{\s*(0x[0-9a-fA-F]+)(?:ull|ULL)?,\s*(0x[0-9a-fA-F]+)(?:ull|ULL)?\s*\}", match.group(1))]
    need(bool(spans) and all(lo < hi for lo, hi in spans), "generated MAIN domain invalid")
    return {**constants, "domain": spans, "generated_main_sha256": digest(source)}


def check_scene_layout(files: dict[str, bytes], layout: dict) -> dict:
    frame = sections(files["frame.bin"])
    addresses = frame.get("addresses", b"")
    need(len(addresses) >= 72, "frame addresses missing")
    stat, damage, main = struct.unpack_from("<QQQ", addresses)
    need(len(frame.get("stat", b"")) == 0x900 and len(frame.get("d0", b"")) == 0xb00, "frame source buffers have wrong sizes")
    need(main + 0x218 == layout.get("MAIN_SCORE"), "scene frame MAIN address differs from compiled MAIN_SCORE")
    domain = layout.get("domain", [])
    for address, size, label in ((stat, 0x900, "stat"), (damage, 0xb00, "damage"), (main, 0x300, "main")):
        need(any(lo <= address and address + size <= hi for lo, hi in domain), "compiled MAIN domain does not cover frame " + label)
    lines = files["hits.txt"].decode("ascii").splitlines()
    need(len(lines) >= 3 and int(lines[0]) > 0, "invalid hit input header")
    addresses_hit = [int(value, 16) for value in lines[1].split()]
    need(bool(addresses_hit), "hit write domain empty")
    need(all(any(lo <= address and address + 4 <= hi for lo, hi in domain) for address in addresses_hit), "hit address outside compiled MAIN domain")
    plan = json.loads(files["event_plan.json"])
    need(plan.get("main_address") == main, "event plan MAIN differs from frame")
    if (source := plan.get("candidate_source_address")) is not None:
        need(type(source) is int and source > 0 and all(
            any(lo <= source + offset and source + offset + 0xc04 <= hi for lo, hi in domain)
            for offset in (0x35d0, 0x4728)), "compiled MAIN does not accept the candidate SOURCE maps")
    need(plan.get("hit_values_sha256") == digest(files["hits.txt"]), "event plan hit values are stale")
    empty_actor_buffers = (len(addresses) == 72 and not any(addresses[56:])
                           and all(name in frame and not any(frame[name])
                                   for name in ("stat", "d0", "s_base", "d_base")))
    if source is not None:
        category = plan.get("candidate_category_field")
        need(type(category) is int and any(lo <= category and category + 4 <= hi for lo, hi in domain),
             "compiled MAIN does not accept the equipped weapon category")
        if (record := plan.get("candidate_inventory_record")) is not None:
            need(type(record) is int and all(
                any(lo <= record + offset and record + offset + 4 <= hi for lo, hi in domain)
                for offset in (0, 0x14, 0x18)), "compiled MAIN does not accept the held item fields")
    templates = plan.get("templates", [])
    fixed_contacts = None
    if plan.get("mode") == {"kind": "fixed_contacts"} and len(templates) == int(lines[0]) and all(
            t.get("scored", True) is True and type(t.get("landed")) is bool and type(t.get("guard")) is bool for t in templates):
        fixed_contacts = sum(t["guard"] and t["landed"] for t in templates)
    return {"stat_address": stat, "damage_address": damage, "main_address": main, "hit_write_addresses": len(addresses_hit), "frame_sections": sorted(frame), "fixed_scored_contacts": fixed_contacts,
            "empty_actor_buffers_and_seeds": empty_actor_buffers}


def pe_exports(raw: bytes) -> set[str]:
    # PE parsing is already a packaging dependency; keeping it lazy means JSON
    # receipt/layout checks and --help need only the Python standard library.
    import pefile
    with pefile.PE(data=raw, fast_load=True) as pe:
        need(pe.FILE_HEADER.Machine == 0x8664, "scorer DLL is not Windows x64")
        pe.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_EXPORT"]])
        return {item.name.decode("ascii") for item in getattr(pe, "DIRECTORY_ENTRY_EXPORT", type("Empty", (), {"symbols": []})()).symbols if item.name}


def verify_scene(files: dict[str, bytes], scene_id: str, app_commit: str | None = None) -> dict:
    need(all(safe_name(name) for name in files), "unsafe scene file path")
    missing = REQUIRED - files.keys()
    need(not missing, "scene " + scene_id + " missing runtime files: " + ", ".join(sorted(missing)))
    unexpected = files.keys() - (REQUIRED | {CONTRACT})
    need(not unexpected, "scene contains non-deployable or stale files: " + ", ".join(sorted(unexpected)))
    need(CONTRACT in files, "scene " + scene_id + " missing " + CONTRACT)
    contract = json.loads(files[CONTRACT])
    need(contract.get("schema") == SCHEMA and contract.get("scene_id") == scene_id, "scene runtime contract identity invalid")
    need(contract.get("files") == inventory(files), "scene file set/bytes differ from bound runtime contract")
    need(contract.get("abi") == ABIS, "scene ABI contract invalid")
    for key in ("app_source_commit", "native_source_commit"):
        need(re.fullmatch(r"[0-9a-f]{40}", str(contract.get(key, ""))) is not None, "invalid contract " + key)
    if app_commit is not None:
        need(contract["app_source_commit"] == app_commit, "runtime contract belongs to another app source commit")
    descriptor = json.loads(files["bundle.json"])
    need(descriptor.get("id") == scene_id, "scene descriptor ID differs from directory")
    exports = {}
    for name, required in EXPORTS.items():
        actual = pe_exports(files[name])
        need(required <= actual, name + " missing required ABI exports: " + ", ".join(sorted(required - actual)))
        exports[name] = sorted(actual)
    layout = check_scene_layout(files, contract["kernel_layout"])
    need(contract.get("compatibility_status") == "host_e2e_required", "a static contract cannot certify host numerical compatibility")
    return {"scene_id": scene_id, "descriptor": descriptor, "files": inventory(files), "contract_sha256": digest(files[CONTRACT]), "layout": layout, "exports": exports, "abi_expected": ABIS, "host_e2e_required": True}


def verify_scene_directory(path: Path, app_commit: str | None = None, scene_id: str | None = None) -> dict:
    need(not any(file.is_symlink() for file in path.rglob("*")), "scene directory contains a symbolic link")
    files = {file.relative_to(path).as_posix(): file.read_bytes() for file in path.rglob("*") if file.is_file()}
    return verify_scene(files, scene_id or path.name, app_commit)


def verify_archive_scenes(archive: zipfile.ZipFile, app_commit: str | None = None) -> list[dict]:
    names = archive.namelist()
    scene_ids = sorted({name[len(SCENE_ROOT):].split("/", 1)[0] for name in names if name.startswith(SCENE_ROOT) and "/" in name[len(SCENE_ROOT):]})
    reports = []
    for scene_id in scene_ids:
        prefix = SCENE_ROOT + scene_id + "/"
        entries = [name for name in names if name.startswith(prefix) and not name.endswith("/")]
        need(len({name.casefold() for name in entries}) == len(entries), "duplicate scene ZIP entry")
        reports.append(verify_scene({name[len(prefix):]: archive.read(name) for name in entries}, scene_id, app_commit))
    return reports


def artifact_report(path: Path) -> dict:
    if path.is_dir():
        manifest = json.loads((path / "build-manifest.json").read_bytes())
        root = path / SCENE_ROOT
        scenes = [verify_scene_directory(scene, manifest["git"]["commit"]) for scene in sorted(root.iterdir()) if scene.is_dir()] if root.is_dir() else []
        return {"artifact_kind": "directory", "source_commit": manifest["git"]["commit"], "scenes": scenes, "passed": True}
    raw = path.read_bytes()
    payload = raw
    kind = "zip"
    if path.suffix.lower() == ".exe":
        need(len(raw) > FOOTER.size, "one-file footer absent")
        magic, length, signature = FOOTER.unpack(raw[-FOOTER.size:])
        need(magic == b"NIOH3_ONEFILE_V1" and 0 < length < len(raw) - FOOTER.size, "one-file footer invalid")
        payload = raw[-FOOTER.size-length:-FOOTER.size]
        need(hashlib.sha256(payload).digest() == signature, "one-file payload hash invalid")
        kind = "onefile"
    with zipfile.ZipFile(io.BytesIO(payload)) as archive:
        manifest = json.loads(archive.read("build-manifest.json"))
        scenes = verify_archive_scenes(archive, manifest["git"]["commit"])
        # Runtime identity is checked independently of the contract: changing
        # both DLL and its contract must still agree with the final manifest.
        declared = {row["path"]: row for row in manifest["files"]}
        for name in archive.namelist():
            if name.startswith(SCENE_ROOT) and not name.endswith("/"):
                body = archive.read(name)
                need(name in declared and declared[name]["sha256"] == digest(body) and declared[name]["size"] == len(body), "scene differs from final build manifest: " + name)
    return {"artifact_kind": kind, "artifact_sha256": digest(raw), "payload_sha256": digest(payload), "source_commit": manifest["git"]["commit"], "scenes": scenes, "passed": True}


def verify_host_receipt(artifact: dict, receipt: dict, scene_id: str | None = None) -> None:
    need(artifact["artifact_kind"] == "onefile", "host receipt must bind the final outer EXE")
    accepted = (receipt.get("release_ready") is True if scene_id is None else receipt.get("scene_player_verified") is True)
    need(accepted and receipt.get("mode") == "player" and receipt.get("private_resources_copied") is False, "engineering/private-fixture evidence cannot authorize a player release")
    fresh = receipt.get("fresh_install", {})
    need(fresh.get("passed") is True and fresh.get("source") == "live" and fresh.get("searches_complete") is True and fresh.get("private_resources_copied") is False and fresh.get("provisioned_by") == "product", "fresh player install has no accepted product provisioning/search")
    need(receipt.get("schema") == HOST_SCHEMA and receipt.get("passed") is True, "packaged numerical host receipt absent or failed")
    need(receipt.get("outer_sha256") == artifact["artifact_sha256"] and receipt.get("source_commit") == artifact["source_commit"], "host receipt belongs to another EXE/commit")
    need(receipt.get("environment", {}).get("platform") == "win32" and receipt.get("runtime_overrides") == [], "host receipt did not use the packaged Windows runtime")
    actual_scenes = {entry["scene_id"]: entry for entry in artifact["scenes"]}
    if scene_id is not None:
        need(scene_id in actual_scenes, "requested scene is absent from the artifact")
        actual_scenes = {scene_id: actual_scenes[scene_id]}
    need(bool(actual_scenes), "optimizer host acceptance requires at least one shipped scene")
    covered = receipt.get("scenes", [])
    need(len(covered) == len(actual_scenes) and {entry.get("scene_id") for entry in covered} == set(actual_scenes), "host receipt does not cover every shipped scene")
    for entry in covered:
        expected = actual_scenes[entry["scene_id"]]
        need("source_rebuild" not in expected["layout"]["frame_sections"],
             "public frame contains a character-bound source program and reference inventory")
        need(expected["layout"].get("empty_actor_buffers_and_seeds") is True,
             "public frame contains character stat, damage or seed values")
        need(entry.get("files") == expected["files"], "host receipt scene bytes differ from EXE")
        runs = entry.get("runs", [])
        need(len(runs) == 2 and {run.get("requested_backend") for run in runs} == {"cpu", "card"}, "host receipt must cover CPU and actual CUDA")
        for run in runs:
            backend = run["requested_backend"]
            need(run.get("actual_backend") == backend, "host receipt reports backend fallback")
            need(all(type(run.get(key)) is int and run[key] >= 0 for key in ("planned", "searched", "answered", "refused", "no_path", "source_uncomputed")), "invalid host numerical counters")
            need(run.get("complete") is True and run.get("answered", 0) > 0, "host numerical search was incomplete or empty")
            need(run.get("searched") == run["answered"] and run.get("planned") == run["answered"], "host search counters do not reconcile")
            need(all(run.get(key) == 0 for key in ("refused", "no_path", "source_uncomputed")), "host search has unanswered candidates")
            executed = run.get("execution_receipt", {})
            need(executed.get("schema") == "nioh3-loadout-execution/v1" and executed.get("scope") == "prepared_exact_sweep", "missing native execution receipt")
            need(executed.get("backend") == backend and executed.get("planned") == run["answered"] and executed.get("completed") == run["answered"], "execution receipt counters differ from search")
            if backend == "card":
                contacts = expected["layout"].get("fixed_scored_contacts")
                need(type(contacts) is int and contacts > 0, "CUDA acceptance needs a fixed scored contact count")
                count = contacts * run["answered"]
                need(executed.get("native_calls", 0) > 0 and executed.get("submitted_contacts") == count and executed.get("completed_contacts") == count and executed.get("failed_batches") == 0 and executed.get("reuse_enabled") is False, "CUDA module did not execute every expected contact")
            else:
                need(executed.get("cpu_executed_candidates", 0) > 0 and executed.get("cpu_executed_candidates", 0) + executed.get("cpu_certified_candidates", 0) == run["answered"], "CPU execution/certificate counts differ from search")
            library = "main_cpu.dll" if backend == "cpu" else "main_device.dll"
            loaded = run.get("loaded_library", {})
            need(loaded.get("sha256") == expected["files"][library]["sha256"], "wrong actual loaded scorer bytes")
            need(loaded.get("scene_files_match_payload") is True and loaded.get("worker_from_payload_cache") is True, "scorer/worker came from an override or unknown path")
            need(run.get("observed_abis", {}).get("nk_context_abi") == 2 and run.get("observed_abis", {}).get("nk_main_" + ("cpu" if backend == "cpu" else "device") + "_components_abi") == 1, "host ABI query did not match")
            need(run.get("winner_components_matched") is True and bool(run.get("winner_component_bits")), "winner channels were not independently matched")
            need(run.get("rank_rows_matched", 0) > 0 and re.fullmatch(r"[0-9a-f]{64}", str(run.get("reference_sha256", ""))) is not None, "host did not independently compare published rankings")
        need(runs[0].get("rank_sha256") == runs[1].get("rank_sha256") and re.fullmatch(r"[0-9a-f]{64}", str(runs[0].get("rank_sha256", ""))) is not None, "packaged CPU/CUDA rankings differ")
    negative = receipt.get("no_private_profile", {})
    need(negative.get("ran") is True and negative.get("false_complete") is False, "first-install missing-profile control absent")


def seal(scene: Path, generated: Path, app_commit: str, native_commit: str) -> dict:
    files = {file.relative_to(scene).as_posix(): file.read_bytes() for file in scene.rglob("*") if file.is_file() and file.name != CONTRACT}
    need(REQUIRED <= files.keys(), "cannot seal an incomplete dual-backend scene")
    layout = native_layout(generated.read_bytes())
    check_scene_layout(files, layout)
    contract = {"schema": SCHEMA, "scene_id": scene.name, "app_source_commit": app_commit, "native_source_commit": native_commit, "files": inventory(files), "abi": ABIS, "kernel_layout": layout, "compatibility_status": "host_e2e_required"}
    files[CONTRACT] = (json.dumps(contract, indent=2, sort_keys=True) + "\n").encode()
    verify_scene(files, scene.name, app_commit)
    (scene / CONTRACT).write_bytes(files[CONTRACT])
    return contract


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    sub = parser.add_subparsers(dest="command", required=True)
    export = sub.add_parser("export-layout")
    export.add_argument("source", type=Path)
    export.add_argument("target", type=Path)
    frame = sub.add_parser("export-frame")
    frame.add_argument("source", type=Path, help="scene directory containing frame.bin and event_plan.json")
    frame.add_argument("target", type=Path)
    certify = sub.add_parser("seal")
    certify.add_argument("scene", type=Path)
    certify.add_argument("--generated-main", type=Path, required=True)
    certify.add_argument("--app-commit", required=True)
    certify.add_argument("--native-commit", required=True)
    check = sub.add_parser("check")
    check.add_argument("artifact", type=Path)
    check.add_argument("--host-receipt", type=Path)
    check.add_argument("--require-host-e2e", action="store_true")
    check.add_argument("--verify-player-scene", help="verify one scene receipt without granting full release acceptance")
    check.add_argument("--output", type=Path)
    args = parser.parse_args()
    try:
        if args.command == "export-layout":
            export_layout(args.source, args.target)
            report = {"passed": True, "layout": str(args.target), "requires_player_context": True}
        elif args.command == "export-frame":
            public = public_frame((args.source / "frame.bin").read_bytes(), json.loads((args.source / "event_plan.json").read_bytes()))
            with args.target.open("xb") as stream:
                stream.write(public)
            report = {"passed": True, "frame": str(args.target), "sha256": digest(public)}
        elif args.command == "seal":
            report = seal(args.scene, args.generated_main, args.app_commit, args.native_commit)
        else:
            report = artifact_report(args.artifact)
            need(bool(report["scenes"]), "optimizer acceptance requires at least one shipped scene")
            if args.verify_player_scene:
                need(args.host_receipt is not None and not args.require_host_e2e, "per-scene check requires its own host receipt")
                verify_host_receipt(report, json.loads(args.host_receipt.read_bytes()), args.verify_player_scene)
                report.update(verified_player_scene=args.verify_player_scene, release_ready=False, host_e2e_passed=False)
            elif args.require_host_e2e or args.host_receipt:
                need(args.host_receipt is not None, "final release requires --host-receipt from packaged numerical E2E")
                verify_host_receipt(report, json.loads(args.host_receipt.read_bytes()))
                report["host_e2e_passed"] = True
                report["release_ready"] = True
            else:
                report["release_ready"] = False
                report["host_e2e_passed"] = False
    except (OSError, ValueError, KeyError, TypeError, struct.error, zipfile.BadZipFile) as error:
        report = {"passed": False, "error": str(error), "exception": type(error).__name__}
    text = json.dumps(report, indent=2, sort_keys=True) + "\n"
    if getattr(args, "output", None):
        args.output.parent.mkdir(parents=True, exist_ok=True)
        args.output.write_text(text, encoding="utf-8")
    print(text, end="")
    return 0 if report.get("passed", args.command == "seal") else 1


if __name__ == "__main__":
    raise SystemExit(main())
