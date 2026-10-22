"""Drive the installed Nioh3Studio protected runtime worker directly (4-byte LE length-prefixed JSON frames).

Launches the packaged nioh3-protected-worker with the same argv the Tauri broker uses for the runtime role,
with its own state root under F:/Nioh3_r16_build/work/studio-state, so the app's own journal is untouched.
argv: <requests.jsonl or -> ; each line {"method": ..., "params": {...}}; prints every reply.
"""
import json
import struct
import subprocess
import sys
import uuid
from pathlib import Path

PKG = Path("C:/Users/oudeb/AppData/Local/Nioh3Studio/onefile/3f90d8b543f01c6ccbbef35f14c43775df7275e698c8ba69d6a8082846d16e31")
RUNTIME = PKG / "worker/runtime"
STATE = Path("F:/Nioh3_r16_build/work/studio-state")


class Worker:
    def __init__(self, role="runtime", version="2.0.2.0"):
        STATE.mkdir(parents=True, exist_ok=True)
        argv = [str(PKG / "worker/nioh3-protected-worker.exe"), "--role", role, "--game-file-version", version,
                "--state-root", str(STATE), "--data-root", str(RUNTIME / "nioh3_scroll_editor/data"),
                "--contract-dir", str(PKG / "packages/contracts"),
                "--accelerator", str(RUNTIME / "bin/nioh3_seed_accelerator.dll")]
        self.p = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.PIPE,
                                  env={**__import__("os").environ, "NIOH3_STATE_ROOT": str(STATE)})

    def call(self, method, params=None):
        body = json.dumps({"protocol": 1, "id": str(uuid.uuid4()), "method": method, "params": params or {}}).encode()
        self.p.stdin.write(struct.pack("<I", len(body)) + body)
        self.p.stdin.flush()
        head = self.p.stdout.read(4)
        if len(head) < 4:
            raise RuntimeError("worker closed: " + self.p.stderr.read().decode(errors="replace")[-2000:])
        n = struct.unpack("<I", head)[0]
        reply = json.loads(self.p.stdout.read(n))
        result = reply.get("result") if isinstance(reply, dict) else None
        if isinstance(result, dict) and result.get("job_id") and result.get("state") == "running" and method != "job.snapshot":
            import time
            while True:  # long operations answer with a job; poll it to completion
                time.sleep(0.5)
                snap = self.call("job.snapshot", {"job_id": result["job_id"]})
                state = (snap.get("result") or {}).get("state")
                if state not in ("running", "queued", "pending"):
                    return snap
        return reply

    def close(self):
        try:
            self.p.stdin.close()
            self.p.wait(timeout=20)
        except Exception:
            self.p.kill()


if __name__ == "__main__":
    import os
    w = Worker(role=os.environ.get("STUDIO_ROLE", "runtime"))
    try:
        print(json.dumps(w.call("handshake"), ensure_ascii=False)[:1500], flush=True)
        lines = sys.stdin.read().splitlines() if sys.argv[1:] == ["-"] else Path(sys.argv[1]).read_text(encoding="utf-8").splitlines()
        for line in lines:
            if line.strip():
                req = json.loads(line)
                reply = w.call(req["method"], req.get("params"))
                if os.environ.get("STUDIO_OUT"):
                    with open(os.environ["STUDIO_OUT"], "a", encoding="utf-8") as stream:
                        stream.write(json.dumps({"request": req, "reply": reply}, ensure_ascii=False) + "\n")
                print(json.dumps(reply, ensure_ascii=False)[:1500], flush=True)
    finally:
        w.close()
