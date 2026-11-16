"""Production framed-worker E2E for the independently certified solo branch.

Uses native-observed sanitized fixtures, a genuine bounded search/filter job,
legacy-version controls and a changed-byte owned resource copy. No game or save
operation is sent. Full frames, binaries/source hashes and results are retained.
"""
from __future__ import annotations

import argparse
import hashlib
import json
import os
import queue
import shutil
import struct
import subprocess
import threading
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]


def save(path: Path, value: object) -> None:
    path.write_text(json.dumps(value, indent=2) + "\n", encoding="utf-8", newline="\n")


def digest(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


class Worker:
    def __init__(self, binary: Path, data: Path, version: str, output: Path):
        output.mkdir()
        self.output, self.frames, self.sequence = output, [], 0
        self.replies: queue.Queue = queue.Queue()
        environment = os.environ.copy()
        environment.update(NIOH3_SEED_BACKEND="cuda", CUDA_VISIBLE_DEVICES="-1")
        environment.pop("NIOH3_RUST_SEARCH_WORKER", None)
        self.errors = (output / "stderr.log").open("wb")
        self.process = subprocess.Popen(
            [str(binary), "--packaged-worker", "--game-file-version", version,
             "--data-root", str(data), "--contract-dir", str(ROOT / "packages" / "contracts"),
             "--accelerator", str(ROOT / "bin" / "nioh3_seed_accelerator.dll")],
            cwd=ROOT, env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.errors,
        )
        threading.Thread(target=self.read, daemon=True).start()

    def read(self) -> None:
        def exact(size: int) -> bytes:
            result = b""
            while len(result) < size:
                part = self.process.stdout.read(size - len(result))
                if not part:
                    raise EOFError("Read-only worker closed")
                result += part
            return result
        try:
            while True:
                size = struct.unpack("<I", exact(4))[0]
                if size > 16 * 1024 * 1024:
                    raise ValueError("Worker frame cap exceeded")
                self.replies.put(json.loads(exact(size)))
        except Exception as error:
            self.replies.put(error)

    def request(self, method: str, params: dict) -> dict:
        self.sequence += 1
        request = {"protocol": 1, "id": f"selector-{self.sequence}", "method": method, "params": params}
        body = json.dumps(request).encode("utf-8")
        self.process.stdin.write(struct.pack("<I", len(body)) + body)
        self.process.stdin.flush()
        reply = self.replies.get(timeout=45)
        if isinstance(reply, Exception):
            raise reply
        self.frames.append({"request": request, "response": reply})
        if reply.get("id") != request["id"] or reply.get("ok") is not True:
            raise RuntimeError(f"{method}: {reply}")
        return reply["result"]

    def preview(self, seed: int) -> dict:
        return self.request("candidate.preview", {"seed": seed, "rarity": 4, "level": 180, "playthrough": 3})["candidate"]

    def finish(self, job: dict) -> dict:
        deadline = time.monotonic() + 45
        while time.monotonic() < deadline:
            current = self.request("job.snapshot", {"job_id": job["job_id"]})
            if current["state"] == "completed":
                return current
            if current["state"] in ("failed", "cancelled"):
                raise RuntimeError(f"Search did not complete: {current}")
            time.sleep(0.05)
        self.request("job.cancel", {"job_id": job["job_id"]})
        raise TimeoutError("Bounded selector filter job timed out")

    def close(self) -> None:
        try:
            if self.process.poll() is None:
                self.request("shutdown", {})
                self.process.stdin.close()
                self.process.wait(timeout=10)
        finally:
            if self.process.poll() is None:
                self.process.kill()  # This owned read-only worker has no game/native mutation owner.
                self.process.wait(timeout=5)
            self.errors.close()
            save(self.output / "frames.json", self.frames)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--worker", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    output = args.output.resolve()
    output.mkdir(parents=True, exist_ok=False)
    proof_path = ROOT / "data" / "enemy_states" / "pc_v2_02" / "selector_one_certification.json"
    proof = json.loads(proof_path.read_text(encoding="utf-8"))
    checks, workers, complete = [], [], False

    def check(name: str, condition: bool, details: object = None) -> None:
        checks.append({"name": name, "pass": bool(condition), "details": details})
        if not condition:
            raise AssertionError(f"{name}: {details}")

    try:
        current = Worker(args.worker.resolve(), ROOT / "data", "2.0.2.0", output / "pc202")
        workers.append(current)
        handshake = current.request("handshake", {})
        check("state-preview revision invalidates old cached contexts",
              handshake["context"]["algorithm_version"] == "scroll-generation-v0.7-native-completion-2")
        for control in proof["native_controls"]:
            candidate = current.preview(control["seed"])
            state = candidate["enemy_states"]["solo"]
            expected = [(item["wave_index"], item["position"], item["spawn_key"], item["lookup_key"],
                         item["role"], "yes" if item["possessed"] else "no") for item in control["occurrences"]]
            actual = [(item["wave_index"], item["position"], item["native_spawn_key"], item["lookup_key"],
                       item["role"], item["possessed"]) for item in state["occurrences"]]
            check(control["id"] + " production occurrences match independently observed native flags",
                  state["possessed_complete"] is True and actual == expected, actual)
            check(control["id"] + " unverified expedition stays unknown",
                  candidate["enemy_states"]["expedition"]["possessed_complete"] is False
                  and not candidate["enemy_states"]["expedition"]["occurrences"])
        reported = current.preview(141974638)["enemy_states"]["solo"]
        check("reported empty-roster ID now has complete solo states",
              reported["possessed_complete"] is True and len(reported["occurrences"]) == 5)
        check("reported seed marks only wave-2 Red Kappa",
              [(item["wave_index"], item["lookup_key"]) for item in reported["occurrences"]
               if item["possessed"] == "yes"] == [(1, 163259)])
        for seed in (86872488, 156062997):
            normal = current.preview(seed)["enemy_states"]["solo"]
            check(f"default-selector control {seed} remains complete", normal["possessed_complete"] is True)
        query = {
            "playthrough": 3, "rarity": 4, "level": 180, "primary_effect_ids": [],
            "required_secondary_ids": [], "required_secondary_id_groups": [], "grace_effect_id": None,
            "grace_effect_ids": [], "minimum_roll_percent_by_effect_id": [], "terrain_selection_ids": [],
            "initial_challenge_counts": [], "enemy_variant": "solo", "effect_occurrences": [],
            "auxiliary": {"required_terrain_effect_keys": [], "required_terrain_effect_key_groups": [],
                          "required_special_rule_keys": [], "required_special_rule_key_groups": [],
                          "required_enemy_lookup_keys": [], "required_enemy_lookup_key_groups": [[163259, 405836]]},
            "enemy_occurrence_groups": [[{"lookup_keys": [163259, 405836], "state": "possessed", "availability": "base"}]],
        }
        result = current.finish(current.request("search.start", {
            "query": query, "context_digest": handshake["context"]["context_digest"],
            "result_count": 3, "allow_cpu_fallback": True, "resume_token": None,
            "continue_until_complete": False, "page_trials": 100000, "job_trials": 100000,
        }))
        check("genuine possession-filter search returns results", len(result["candidates"]) == 3)
        # Independently replayed as descriptor selector 1, not merely another
        # hit from the already-supported default branch.
        check("filter reaches the selector-1 regression control",
              any(candidate["seed"] == 31611131 for candidate in result["candidates"]))
        for candidate in result["candidates"]:
            actual = current.preview(candidate["seed"])["enemy_states"]["solo"]
            check(f"filtered result {candidate['seed']} really contains a possessed Red Kappa",
                  actual["possessed_complete"] is True and any(
                      item["lookup_key"] in (163259, 405836) and item["possessed"] == "yes"
                      for item in actual["occurrences"]))
        legacy = Worker(args.worker.resolve(), ROOT / "data", "2.0.1.0", output / "pc201")
        workers.append(legacy)
        legacy.request("handshake", {})
        legacy_state = legacy.preview(100000439)["enemy_states"]["solo"]
        check("PC v2.01 does not inherit the new native certification",
              legacy_state["possessed_complete"] is False and not legacy_state["occurrences"])
        changed_data = output / "owned-changed-data"
        shutil.copytree(ROOT / "data", changed_data)
        table_path = changed_data / "enemy_states" / "pc_v2_01" / "native_tables.json"
        with table_path.open("ab") as handle:
            handle.write(b"\n")  # Same JSON values, different proof-bound file bytes.
        changed = Worker(args.worker.resolve(), changed_data, "2.0.2.0", output / "changed-table")
        workers.append(changed)
        changed.request("handshake", {})
        changed_state = changed.preview(100000439)["enemy_states"]["solo"]
        check("changed table bytes remove selector-1 certification",
              changed_state["possessed_complete"] is False and not changed_state["occurrences"])
        check("changed-byte control preserves the existing default path",
              changed.preview(156062997)["enemy_states"]["solo"]["possessed_complete"] is True)
        complete = True
    finally:
        for worker in reversed(workers):
            worker.close()
        save(output / "report.json", {"pass": complete and bool(checks) and all(item["pass"] for item in checks),
             "scope": "Actual read-only workers and bounded CPU search; native fixture comparisons; no game/save access",
             "checks": checks, "worker_sha256": digest(args.worker), "native_fixture_sha256": digest(proof_path)})
    print(json.dumps({"pass": True, "checks": len(checks), "output": str(output)}))


if __name__ == "__main__":
    main()
