"""Demand-driven MAIN replay: fill each actual missing read from the same live PID, read-only.

Every added cell is labelled: module image (static) or heap read *after* the hit.
Heap cells are an explicit post-hit assumption (valid only for state unchanged
since the hit, e.g. startup-loaded parameter tables); the receipt lists them all.
Stops on normal return, a non-memory dependency, a demand inside captured
objects, or the iteration budget.
"""
import argparse
import json
import sys
import time
from pathlib import Path

ROOT = Path("F:/Nioh3_ScrollEditor")
REPLAY_DIR = Path("F:/Nioh3_r16_build/deliverables/pro-r2-return/nioh3-damage-formula-r2-pro-return-pc202-20261018/project-source/runtime/project-source/r13-main")
IMAGE_SIZE = 77830112
WINDOW = 0x40  # unused: exact demanded spans only
# Readers inside the startup-loaded parameter-table lookup (2FA678 family) and
# its binary search (D24926..D24932); see live-hit-02-fill-* FILLED_CELLS.json.
STATIC_TABLE_READERS = {"0xd24926", "0xd2492e", "0xd24932", "0x2fa68b", "0x2fa692", "0x2fa7e9", "0x2fa7f7",
                        "0x2fa802", "0x2fa80e", "0x2fa81a", "0x2fa827", "0x2fa82f", "0x2fa837", "0xffcc46", "0x9819f9"}


def install_getter_cut(replay):
    # The normal nonnull-D getter cut is caller-independent apart from its
    # return-slot check; admit further callers only when their original bytes
    # are a direct CALL 0x624518. Every admitted site is recorded.
    original_cut = replay.getter_cut
    admitted_sites = []

    def getter_cut(parent):
        if parent.call_stack:
            site = parent.call_stack[-1]["site"]
            if site not in parent.getter_sites:
                row = parent.source.instruction(int(site, 16))
                if row["mnemonic"] == "call" and row.get("branch_target") == "0x624518":
                    parent.getter_sites.add(site)
                    if site not in admitted_sites:
                        admitted_sites.append(site)
        incoming = parent.regget("rcx")
        if parent.pc == 0x62451E and incoming.known == (1 << 64) - 1 and incoming.value:
            return default_preserving_cut(parent, incoming.value)
        return original_cut(parent)

    def default_preserving_cut(parent, stat):
        """Getter cut joining at 624541 for both nonnull and NULL Q(STAT+7C0).

        Differs from the retained cut only in not invalidating the default D
        object (base+4BA66B0): its thread-safe static init ran long before the
        hit, so it is mapped from a same-PID post-hit read (explicit assumption).
        """
        if parent.call_stack[-1]["target"] != "0x624518" or parent.call_stack[-1]["site"] not in parent.getter_sites:
            return original_cut(parent)
        from main_program import W, BindingError
        from getter_projection import invalidate_known
        site = parent.call_stack[-1]["site"]
        sp = parent.regget("rsp").exact()
        if parent.mem.read(sp + 0x28, 8).exact() != parent.base + int(site, 16) + 5:
            raise BindingError("Getter actual CALL and return-slot identity disagree")
        stack_start = parent.scene["stack_extent"]["start_u64"]
        havoc = invalidate_known(parent.mem, stack_start, sp + 0x20, "fresh_normal_getter_unknown_runtime_stack")
        for rva, size in ((0x4BA66A0, 4), (0x44B7F20, 4)):
            havoc.extend(invalidate_known(parent.mem, parent.base + rva, parent.base + rva + size,
                                          "fresh_normal_getter_unknown_TLS_epoch"))
        for name in ("rax", "rcx", "rdx", "r8", "r9", "r10", "r11"):
            parent.regset(name, W.unk(64, "fresh_getter_volatile:" + name))
        for i in range(6):
            parent.regset("xmm" + str(i), W.unk(128, "fresh_getter_volatile:xmm" + str(i)))
        parent.regset("rbx", W.val(stat))
        parent.flags = {key: None for key in parent.flags}
        parent.pc = 0x624541
        parent.steps += 1
        selected = parent.mem.read(stat + 0x7C0, 8)
        null = selected.known == (1 << 64) - 1 and selected.value == 0
        if null:
            null_sites.append(site)
        return dict(kind="fresh_D_getter_cut_default_preserved", null_D=null, entry_rva="0x62451e",
                    join_rva="0x624541", call_site_rva=site, STAT_u64=stat,
                    D_u64=parent.base + 0x4BA66B0 if null else selected.value, abstract_havoc=havoc,
                    default_D_contents="same-PID post-hit read; static default assumed initialized and unchanged")

    null_sites = []
    replay.getter_cut = getter_cut
    return admitted_sites, null_sites


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--capture", type=Path, required=True)
    parser.add_argument("--cache-dir", type=Path, required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    parser.add_argument("--port", type=int, default=5566)
    parser.add_argument("--session-id")
    parser.add_argument("--offline", action="store_true", help="no CE bridge: every missing read stops the run")
    parser.add_argument("--iterations", type=int, default=60)
    parser.add_argument("--resume-request", type=Path, help="ENTRY_REQUEST.json from an earlier fill run")
    parser.add_argument("--prefetch", type=Path, nargs="*", default=[], help="FILLED_CELLS.json files from the same PID")
    args = parser.parse_args()
    out = args.output_dir
    out.mkdir(parents=True, exist_ok=False)
    sys.path.insert(0, str(REPLAY_DIR))
    import replay
    sys.path.insert(0, str(ROOT / ".tools/cheat-engine-mcp/python/src"))
    from ce_mcp_server.bridge import CheatEngineBridge
    admitted_sites, null_sites = install_getter_cut(replay)

    request = (json.loads(args.resume_request.read_bytes()) if args.resume_request
               else replay.prepare_request(args.capture, args.cache_dir))
    scene = request["scene"]
    base = scene["module_base_u64"]
    pid = json.loads(args.capture.read_bytes())["identity"]["process_id"]
    bridge = None if args.offline else CheatEngineBridge(host="127.0.0.1", port=args.port)
    if bridge:
        bridge.start()
    log = (out / "LOOP.jsonl").open("w", encoding="utf-8")

    def read(address, size):
        if bridge is None:
            return None
        script = ("local t=readBytes(%d,%d,true); local h={} for i,v in ipairs(t or {}) do h[i]=string.format('%%02x',v) end;"
                  "return {pid=getOpenedProcessID(),base=getAddressSafe('Nioh3.exe'),hex=table.concat(h)}") % (address, size)
        r = bridge.call_tool("ce.lua_exec", {"script": script}, args.session_id, 30.0)
        value = r.get("result", r)
        if value.get("pid") != pid or value.get("base") != base:
            raise RuntimeError("CE is no longer attached to the captured process")
        data = bytes.fromhex(value.get("hex") or "")
        return data if len(data) == size else None

    def add(address, size, reason):
        # Exact demanded span only: wider windows can change earlier projected cuts.
        start, end = address, address + size
        data = read(start, size)
        if data is None:
            return 0
        added = 0
        for at in range(start, end, 8):
            chunk = data[at - start:at - start + 8]
            overlap = False
            for r in scene["memory_regions"]:
                lo = max(at, r["address_u64"])
                hi = min(at + len(chunk), r["address_u64"] + len(r["value_hex"]) // 2)
                if lo < hi:
                    overlap = True
                    break
            if overlap:
                continue
            static = base <= at < base + IMAGE_SIZE
            scene["memory_regions"].append({
                "name": ("static_image_" if static else "posthit_heap_") + hex(at), "address_u64": at,
                "value_hex": chunk.hex(), "known_hex": "ff" * len(chunk), "writable": True,
                "provenance": {"grade": "same-PID read-only after the hit", "module_image": static,
                               "demand_reader_rva": reason, "pid": pid}})
            added += 1
        return added

    try:
        for _ in range(0 if bridge is None else 60):
            if any(s.get("session_id") == args.session_id for s in bridge.list_sessions()):
                break
            time.sleep(1)
        # Prefetch addresses previously demanded by static-table readers only
        # (parameter-table lookups, the default D object, one rdata vtable slot).
        # Values are re-read now from the live PID; dynamic state is never prefetched.
        prefetched = 0
        default_d = (base + 0x4BA66B0, base + 0x4BA66B0 + 0xB00)
        for path in args.prefetch:
            for cell in json.loads(Path(path).read_bytes()):
                address, size = cell["address_u64"], len(cell["value_hex"]) // 2
                reader = cell["provenance"]["demand_reader_rva"].split(":")[0]
                static_reader = reader in STATIC_TABLE_READERS
                in_default = default_d[0] <= address < default_d[1]
                if (static_reader or in_default) and cell["provenance"]["pid"] == pid:
                    prefetched += add(address, size, reader + ":prefetch")
        print(json.dumps({"prefetched_cells": prefetched}), flush=True)
        result = None
        for iteration in range(1, args.iterations + 1):
            result = replay.run_request(request, args.cache_dir)
            native, missing = result["native"], result["last_missing_read"]
            row = {"iteration": iteration, "status": native["status"], "next_rva": native["next_rva"],
                   "steps": native["steps"], "dependency": native.get("dependency"), "missing": missing,
                   "normal_return": result["normal_MAIN_return"], "damage": result["computed_damage_f32"]}
            print(json.dumps(row), flush=True)
            log.write(json.dumps(row) + "\n"); log.flush()
            dep = native.get("dependency") or {}
            if result["normal_MAIN_return"] or dep.get("kind") != "unmapped_memory":
                break
            if not missing:  # an unmapped store: map the destination's current bytes, writable
                missing = {"address_u64": int(dep["address"], 16), "width_bytes": dep["size"],
                           "reader_rva": native["next_rva"] + ":store"}
            if add(missing["address_u64"], missing["width_bytes"], missing["reader_rva"]) == 0:
                print(json.dumps({"stop": "demand overlaps captured memory or is unreadable", "missing": missing}), flush=True)
                break
        (out / "ENTRY_REQUEST.json").write_text(json.dumps(request) + "\n")
        (out / "MAIN.json").write_text(json.dumps(result, indent=2) + "\n")
        filled = [r for r in scene["memory_regions"] if r["name"].startswith(("static_image_", "posthit_heap_"))]
        (out / "FILLED_CELLS.json").write_text(json.dumps(filled, indent=1) + "\n")
        (out / "ADMITTED_GETTER_SITES.json").write_text(json.dumps(admitted_sites) + "\n")
        print(json.dumps({"admitted_getter_sites": admitted_sites, "filled_static": sum(r["provenance"]["module_image"] for r in filled),
                          "filled_heap": sum(not r["provenance"]["module_image"] for r in filled)}), flush=True)
    finally:
        if bridge:
            bridge.stop()


if __name__ == "__main__":
    main()
