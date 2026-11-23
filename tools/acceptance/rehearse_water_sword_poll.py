"""Rehearse ordinary Water Sword sampling on a separate local fixture and retained native records."""

import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import time

import capture_water_sword_poll as poll


def bits(value):
    return int.from_bytes(struct.pack("<f", value), "little")


class OwnedFixture:
    """Only writes Python-owned buffers in this test process."""

    def __init__(self):
        self.buffers = {}
        for key, size in dict(
            cell=8,
            root=0x1340,
            actor=0x500,
            owner=0x4000,
            stat=0x900,
            descriptor=0x100,
            controller=0x100,
            state=0x1800,
            context=0x50,
            action=336,
            flag=8,
            damage=0x900,
        ).items():
            self.buffers[key] = ctypes.create_string_buffer(size)
        for name, off, child in (
            ("cell", 0, "root"),
            ("root", 0x1338, "actor"),
            ("actor", 0x3A0, "owner"),
            ("owner", 0, "actor"),
            ("actor", 0x3B0, "controller"),
            ("controller", 8, "state"),
            ("state", 0xE0, "actor"),
            ("state", 0xE8, "context"),
            ("context", 0x20, "action"),
            ("owner", 0x2D90, "stat"),
            ("stat", 8, "owner"),
            ("stat", 0x7C0, "damage"),
            ("actor", 0xE8, "descriptor"),
            ("controller", 0x40, "flag"),
        ):
            self.put(name, off, "Q", self.addr(child))
        self.put("descriptor", 0x68, "I", 1 << 31)
        self.put("stat", 0x1C, "I", 1220)
        self.put("stat", 0x79C, "I", 20)
        self.put("owner", 0xAC, "f", 1220.0)
        for off in (0x2E80, 0x2E84, 0x2E88):
            self.put("owner", off, "f", 1.0)

    def addr(self, name):
        return ctypes.addressof(self.buffers[name])

    def put(self, name, offset, fmt, value):
        struct.pack_into("<" + fmt, self.buffers[name], offset, value)

    def snapshot(self):
        kernel = ctypes.WinDLL("kernel32", use_last_error=True)
        kernel.GetCurrentProcess.restype = wintypes.HANDLE
        times = [wintypes.FILETIME() for _ in range(4)]
        kernel.GetProcessTimes.argtypes = [wintypes.HANDLE] + [
            ctypes.POINTER(wintypes.FILETIME)
        ] * 4
        if not kernel.GetProcessTimes(
            kernel.GetCurrentProcess(), *(ctypes.byref(t) for t in times)
        ):
            raise ctypes.WinError(ctypes.get_last_error())
        creation = times[0].dwLowDateTime | times[0].dwHighDateTime << 32
        return dict(
            process=dict(pid=os.getpid(), creation_filetime=creation),
            roots={
                k: self.addr(n)
                for k, n in (("player", "actor"), ("owner", "owner"), ("stat", "stat"))
            },
            module_base=self.addr("cell") - poll.PLAYER_GLOBAL,
            inputs={"style": 0},
        )


def wait_for(path, seconds):
    until = time.monotonic() + seconds
    while time.monotonic() < until:
        if path.exists():
            return
        time.sleep(0.01)
    raise TimeoutError(str(path))


def serve(root, folder):
    mem = OwnedFixture()
    params = poll.water_actions(root)
    poll.write(folder / "snapshot.tmp", mem.snapshot())
    (folder / "snapshot.tmp").replace(folder / "snapshot.json")
    wait_for(folder / "window/ARMED.json", 18)
    time.sleep(0.12)
    # A controlled complete action route including later II phases and the finish.
    # Values are fixture inputs, never a retained observation of Water Sword.
    path = [25329, 25330, 25331, 25332, 25333, 25334, 25335, 25336, 25337]
    full_path = path + [25336, 25337] * 5 + [25336]
    balance = 1220.0
    for n, action in enumerate(full_path):
        cost = (
            152.2517547607422
            if n == 0
            else 53.288116455078125
            if n % 2
            else 60.90070343017578
        )
        mem.buffers["action"].raw = params[action]
        mem.put("state", 0x100C, "f", cost)
        time.sleep(0.045)
        balance = poll.f32(balance - cost)
        mem.put("owner", 0xAC, "f", balance)
        time.sleep(0.055)
    mem.buffers["action"].raw = params[poll.FINISH]
    mem.put("state", 0x100C, "f", 200.0)
    before_finish = balance
    for spent in (0.0, 70.0, 140.0, 200.0):
        balance = poll.f32(before_finish - spent)
        mem.put("owner", 0xAC, "f", balance)
        time.sleep(0.06)
    mem.buffers["action"].raw = bytes(336)
    poll.write(
        folder / "stimulus.json",
        dict(actions=full_path + [poll.FINISH], last_balance=balance),
    )
    wait_for(folder / "window/RESULT.json", 10)


def historical(root):
    base = root / "deliverables/ki-write-live-20261113"
    capture = (base / "session04/capture.json").read_bytes()
    oracle = json.loads((base / "FULL_CAST_EVIDENCE.json").read_bytes())
    if poll.digest(capture) != oracle["capture_sha256"]:
        raise ValueError("Retained capture identity differs")
    records = json.loads(capture)["raw_capture"]["records"]
    resource = (
        root
        / "deliverables/event-graph-20261111/matched-resources/archive-01-entry-00193.bin"
    )
    reaper = {int(line["action"], 16) for line in oracle["cast"]}
    hashes = {
        struct.unpack_from("<H", r, 12)[0]: poll.digest(r)
        for r in poll.load_actions(resource)
        if struct.unpack_from("<H", r, 12)[0] in reaper
    }
    samples = []
    for reference in oracle["cast"]:
        r = records[reference["record"] - 1]
        regions = {b["role"]: bytes.fromhex(b["bytes_hex"]) for b in r["regions"]}
        action = regions["source.action_parameter"]
        action_id = struct.unpack_from("<H", action, 12)[0]
        cost = struct.unpack_from("<I", regions["source.state"], 0x100C)[0]
        balance = struct.unpack_from("<I", regions["source.hp"], 0xAC)[0]
        if (
            balance != r["after_bits"]
            or cost != bits(-reference["native_delta"])
            or action_id != int(reference["action"], 16)
            or poll.digest(action) != hashes[action_id]
        ):
            raise ValueError("Original bytes disagree with retained writer evidence")
        samples.append(
            dict(
                action=action_id,
                digest=hashes[action_id],
                ki_bits=balance,
                cost_bits=cost,
                ms=r["elapsed_ms"],
            )
        )

    def idle(ms, ki):
        return dict(action=-1, digest="", ki_bits=bits(ki), cost_bits=0, ms=ms)

    samples.insert(0, idle(samples[0]["ms"] - 1, oracle["cast"][0]["before"]))
    samples.append(idle(samples[-1]["ms"] + 1, oracle["cast"][-1]["after"]))
    diagnostic = poll.summarize(
        samples,
        hashes,
        1220,
        entry=0x6268,
        edges={0x6268: {0x6269}, 0x6269: {0x626A}, 0x626A: {0x6269}},
        upgrade=-2,
        finish=-3,
    )
    if not diagnostic["observations_reconcile"] or len(diagnostic["actions"]) != len(
        oracle["cast"]
    ):
        raise ValueError("Historical native charge sequence did not reconcile")
    return dict(
        records=len(oracle["cast"]),
        first_source=oracle["cast"][0],
        last_source=oracle["cast"][-1],
        all_action_rows_match_resource_sha256=True,
        all_debits_match_ki_writer=True,
        capture_sha256=poll.digest(capture),
        resource_sha256=poll.digest(resource.read_bytes()),
        scope="The 23 writer events are historical Ninja Reaper observations. Idle bookends use the separately recorded starting/ending balance; no sampling-rate inference.",
    )


def rehearse(root, output):
    output.mkdir(parents=True, exist_ok=False)
    runner = Path(poll.__file__)

    def child(script, *args):
        return [sys.executable, "-B", str(script), "--root", str(root), *map(str, args)]

    capture = output / "window"
    server = subprocess.Popen(
        child(Path(__file__), "--serve", output),
        creationflags=subprocess.CREATE_NO_WINDOW,
    )
    try:
        snapshot = output / "snapshot.json"
        wait_for(snapshot, 10)
        preflight = subprocess.run(
            child(runner, "preflight", "--snapshot", snapshot),
            capture_output=True,
            text=True,
            timeout=10,
        )
        if preflight.returncode:
            raise RuntimeError(preflight.stderr)
        result = subprocess.run(
            child(
                runner,
                "capture",
                "--snapshot",
                snapshot,
                "--output",
                capture,
                "--seconds",
                12,
            ),
            capture_output=True,
            text=True,
            timeout=20,
        )
        if result.returncode:
            raise RuntimeError(result.stdout + result.stderr)
        server.wait(timeout=4)
        if server.returncode:
            raise RuntimeError("Fixture failed")
    finally:
        if server.poll() is None:
            server.terminate()
            server.wait(timeout=3)
    live = json.loads((capture / "RESULT.json").read_bytes())
    expected = json.loads((output / "stimulus.json").read_bytes())
    if (
        live["actions"] != expected["actions"]
        or not live["observations_reconcile"]
        or not live["observed_ii_entry"]
        or live["segments"][-1]["debit_evidence"]
        != "split_finish_balance_matches_f32_net_charge"
    ):
        raise ValueError(
            f"Cross-process sampling failed: {live['unresolved']}; got {live['actions']}"
        )
    rows = json.loads((capture / "SAMPLES.json").read_bytes())
    finish = live["segments"][-1]
    first_partial = finish["decreases"][0]["after"]
    partial_row = next(
        i
        for i, r in enumerate(rows)
        if r.get("action") == poll.FINISH and r.get("ki_bits") == bits(first_partial)
    )
    expected_hashes = {i: poll.digest(r) for i, r in poll.water_actions(root).items()}
    loop = next(i for i, b in enumerate(live["segments"]) if b["action"] == 25336)
    left = live["segments"][loop]["from_ms"]
    right = live["segments"][loop + 2]["from_ms"]
    controls = {
        "lost-whole-alternating-pair": (
            [r for r in rows if not left <= r["ms"] < right],
            "missing_or_nonmatching_charge",
        ),
        "lost-intermediate-action": (
            [r for r in rows if r.get("action") != 25330],
            "skipped_or_unexpected_transition",
        ),
        "missing-first-action": (
            [r for r in rows if r.get("action") != 25329],
            "no_entry",
        ),
        "other-resource-with-same-id": (
            [dict(r, digest="other") if r.get("action") == 25332 else r for r in rows],
            "action_number_with_other_parameter_bytes",
        ),
        "wrong-adjusted-cost": (
            [
                dict(r, cost_bits=bits(1.0)) if r.get("action") == 25332 else r
                for r in rows
            ],
            "missing_or_nonmatching_charge",
        ),
        "partial-poll-window": (
            [r for r in rows if r.get("action") in {0, 25329, 25330, 25331}],
            "no_observed_exhausting_charge",
        ),
        "partial-finish-debit": (
            [
                r
                for r in rows
                if r.get("action") != poll.FINISH
                or r["ki_bits"] != bits(expected["last_balance"])
            ],
            "missing_or_nonmatching_charge",
        ),
        "finish-with-intervening-recovery": (
            rows[: partial_row + 1]
            + [dict(rows[partial_row], ki_bits=bits(first_partial + 5.0))]
            + rows[partial_row + 1 :],
            "missing_or_nonmatching_charge",
        ),
    }
    for name, (bad, diagnosis) in controls.items():
        if diagnosis not in poll.summarize(bad, expected_hashes, 1220)["unresolved"]:
            raise ValueError(f"Undetected {name}")
    receipt = dict(
        schema="pc202-water-sword-poll-rehearsal/v1",
        game_opened=False,
        ce_used=False,
        code_sha256=poll.digest(runner.read_bytes()),
        native_history=historical(root),
        host_fixture=dict(
            real_windows_process_read=True,
            process=json.loads(preflight.stdout)["process"],
            path=live["actions"],
            elapsed_sampling_gap_ms=live["max_stable_sample_gap_ms"],
            exit_balance=expected["last_balance"],
            observed_ii=live["observed_ii_entry"],
            discarded_sweeps=live["discarded_reads_during_cast"],
        ),
        fault_controls={k: reason for k, (_, reason) in controls.items()},
        limit="Windows fixture scheduling and historical writer callbacks do not establish a real Water Sword polling frequency, contact count, or held-button state.",
    )
    poll.write(output / "REHEARSAL.json", receipt)
    print(
        json.dumps(
            {
                "passed": True,
                "historical_debits": receipt["native_history"]["records"],
                "fixture_steps": len(live["actions"]),
                "controls": len(controls),
            }
        )
    )


if __name__ == "__main__":
    p = argparse.ArgumentParser(description=__doc__)
    p.add_argument("--root", type=Path, default=poll.BUILD_ROOT)
    mode = p.add_mutually_exclusive_group(required=True)
    mode.add_argument("--serve", type=Path)
    mode.add_argument("--output", type=Path)
    args = p.parse_args()
    if args.serve:
        serve(args.root, args.serve)
    else:
        rehearse(args.root, args.output)
