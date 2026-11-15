"""Exercise the actual packaged-mode offline worker on real GPU adapters.

No game/save operation is sent. Evidence contains each real request/response,
continuation checkpoints, strict-GPU failures and candidate identity checks.
"""
from __future__ import annotations
import argparse
import hashlib
import json
import os
from pathlib import Path
import queue
import struct
import subprocess
import sys
import threading
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / "tools" / "tests"))
from cargo_target import cargo_target_dir, build_root


class Worker:
    def __init__(self, binary: Path, vendor: int, evidence: Path):
        self.evidence, self.sequence, self.frames = evidence, 0, []
        self.replies = queue.Queue()
        environment = os.environ.copy()
        environment["NIOH3_SEED_BACKEND"] = "directcompute"
        environment["NIOH3_D3D11_VENDOR"] = str(vendor)
        environment.pop("NIOH3_RUST_SEARCH_WORKER", None)
        self.errors = (evidence / "worker.stderr.log").open("wb")
        self.process = subprocess.Popen([str(binary), "--packaged-worker", "--game-file-version", "2.0.2.0", "--data-root", str(ROOT / "data"), "--contract-dir", str(ROOT / "packages/contracts"), "--accelerator", str(ROOT / "bin/nioh3_seed_accelerator.dll")], cwd=ROOT, env=environment, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=self.errors)
        threading.Thread(target=self._read, daemon=True).start()

    def _read_exact(self, count):
        data = b""
        while len(data) < count:
            piece = self.process.stdout.read(count-len(data))
            if not piece:
                raise EOFError("worker closed its framed output")
            data += piece
        return data

    def _read(self):
        try:
            while True:
                count = struct.unpack("<I", self._read_exact(4))[0]
                if count > 16*1024*1024:
                    raise RuntimeError("oversized worker frame")
                self.replies.put(json.loads(self._read_exact(count)))
        except Exception as error:
            self.replies.put(error)

    def request(self, method, params):
        self.sequence += 1
        request = {"protocol": 1, "id": f"gpu-{self.sequence}", "method": method, "params": params}
        body = json.dumps(request).encode()
        self.process.stdin.write(struct.pack("<I", len(body))+body)
        self.process.stdin.flush()
        reply = self.replies.get(timeout=30)
        if isinstance(reply, Exception):
            raise reply
        self.frames.append({"request": request, "response": reply})
        if reply.get("id") != request["id"] or not reply.get("ok"):
            raise RuntimeError(f"{method} refused: {reply}")
        return reply["result"]

    def wait(self, job, terminal=True, minimum_cursor=0):
        deadline = time.monotonic()+60
        while time.monotonic() < deadline:
            result = self.request("job.snapshot", {"job_id": job["job_id"]})
            if result["state"] == "failed":
                raise RuntimeError(f"search failed: {result['error']}")
            if terminal and result["state"] in ("completed", "cancelled"):
                return result
            if not terminal and result["cursor"] >= minimum_cursor and result["state"] in ("running", "queued"):
                return result
            time.sleep(0.05)
        raise TimeoutError("search did not reach its required real snapshot")

    def close(self):
        if self.process.poll() is None:
            self.process.terminate()
        self.process.wait(timeout=10)
        self.errors.close()
        (self.evidence / "frames.json").write_text(json.dumps(self.frames, ensure_ascii=False, indent=2)+"\n", encoding="utf-8")


def feedback_query():
    return {"playthrough": 3, "rarity": 4, "level": 180, "primary_effect_ids": [], "required_secondary_ids": [], "required_secondary_id_groups": [], "grace_effect_id": None, "grace_effect_ids": [], "minimum_roll_percent_by_effect_id": [], "auxiliary": {"required_terrain_effect_keys": [], "required_terrain_effect_key_groups": [], "required_special_rule_keys": [], "required_special_rule_key_groups": [[18464], [113], [32497]], "required_enemy_lookup_keys": [], "required_enemy_lookup_key_groups": [[264763], [586715]]}, "terrain_selection_ids": ["exact:"], "initial_challenge_counts": [], "enemy_variant": "solo", "enemy_occurrence_groups": [[{"lookup_keys": [264763], "state": "any", "availability": "any"}], [{"lookup_keys": [586715], "state": "any", "availability": "any"}]], "effect_occurrences": []}


def check(binary, vendor, output):
    output.mkdir(parents=True, exist_ok=True)
    worker = Worker(binary, vendor, output)
    try:
        handshake = worker.request("handshake", {})
        assert handshake["capabilities"]["directcompute_pivot_and_auxiliary"]
        context = handshake["context"]["context_digest"]
        parameters = {"query": feedback_query(), "context_digest": context, "result_count": 25, "allow_cpu_fallback": False, "resume_token": None, "continue_until_complete": True, "page_trials": 100000000, "job_trials": 10000000}
        job = worker.request("search.start", parameters)
        worker.wait(job, terminal=False, minimum_cursor=1000000)
        worker.request("job.cancel", {"job_id": job["job_id"]})
        cancelled = worker.wait(job)
        first_cancel_cursor = cancelled["cursor"]
        assert cancelled["state"] == "cancelled" and cancelled["cursor"] > 0 and cancelled["resume_token"]
        parameters["resume_token"] = cancelled["resume_token"]
        resumed = worker.request("search.start", parameters)
        assert resumed["start_cursor"] == cancelled["cursor"]
        worker.wait(resumed, terminal=False, minimum_cursor=cancelled["cursor"]+1000000)
        worker.request("job.cancel", {"job_id": resumed["job_id"]})
        second = worker.wait(resumed)
        resumed_cancel_cursor = second["cursor"]
        assert second["state"] == "cancelled" and second["cursor"] > cancelled["cursor"]
        first_ids = {candidate["candidate_id"] for candidate in cancelled["candidates"]}
        second_ids = {candidate["candidate_id"] for candidate in second["candidates"]}
        assert not first_ids.intersection(second_ids)
        common = feedback_query()
        common["auxiliary"]["required_special_rule_key_groups"] = [[113]]
        common["auxiliary"]["required_enemy_lookup_key_groups"] = []
        common["terrain_selection_ids"] = []
        common["enemy_occurrence_groups"] = []
        parameters.update(query=common, result_count=1, resume_token=None, continue_until_complete=False, page_trials=200000, job_trials=400000)
        first = worker.wait(worker.request("search.start", parameters))
        assert first["candidates"] and first["resume_token"]
        parameters["resume_token"] = first["resume_token"]
        second = worker.wait(worker.request("search.start", parameters))
        assert second["start_cursor"] == first["cursor"] and second["candidates"]
        assert {c["candidate_id"] for c in first["candidates"]}.isdisjoint(c["candidate_id"] for c in second["candidates"])
        for candidate in first["candidates"]+second["candidates"]:
            preview = worker.request("candidate.preview", {"seed": candidate["seed"], "rarity": candidate["rarity"], "level": 180, "playthrough": 3})
            assert preview["candidate"]["candidate_id"] == candidate["candidate_id"]
        return {"vendor": vendor, "passed": True, "feedback_cancel_cursor": cancelled["cursor"], "feedback_resumed_cursor": resumed_cancel_cursor, "positive_candidates": len(first["candidates"])+len(second["candidates"]), "strict_gpu": True}
    finally:
        worker.close()


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, default=build_root()/"deliverables/directcompute-seed-20261115/worker-e2e")
    parser.add_argument("--vendors", default="4318,4098")
    args = parser.parse_args()
    args.output.mkdir(parents=True, exist_ok=True)
    binary = cargo_target_dir("tauri-target")/"release/nioh3-readonly-worker.exe"
    results = []
    try:
        for vendor in [int(value, 0) for value in args.vendors.split(",")]:
            print(f"WORKER_GPU_E2E_START vendor=0x{vendor:04X}", flush=True)
            results.append(check(binary, vendor, args.output/f"vendor-{vendor:04x}"))
            print(f"WORKER_GPU_E2E_DONE vendor=0x{vendor:04X}", flush=True)
    except Exception as error:
        results.append({"passed": False, "error": str(error)})
    report = {"schema": "nioh3-gpu-worker-e2e/v1", "passed": all(result["passed"] for result in results), "worker_sha256": hashlib.sha256(binary.read_bytes()).hexdigest(), "results": results}
    (args.output/"summary.json").write_text(json.dumps(report, indent=2)+"\n", encoding="utf-8")
    print(json.dumps(report), flush=True)
    return 0 if report["passed"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
