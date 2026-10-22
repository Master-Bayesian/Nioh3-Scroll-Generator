"""Drive the v5 stdin observer runner: preflight, arm, wait for one hit, export, stop.

Progress lines go to <output>/../<name>.STATE.txt so the owner can be told
exactly when the capture window is open. Stop/cleanup always runs.
"""
import argparse
import json
import subprocess
import sys
import time
from pathlib import Path

RUNNER = Path(__file__).resolve().parents[1] / "observer-v5/ce_stdin_runner.py"


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--mode", choices=("mock", "live"), required=True)
    parser.add_argument("--pid", type=int, required=True)
    parser.add_argument("--port", type=int, default=5566)
    parser.add_argument("--session-id")
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--wait-seconds", type=int, default=600)
    # Training grounds refill HP and rebuild the player's stats every frame, so CORE_cut and HP_after_store
    # fire about three times per frame without any attack; the default 384-callback budget ends the window
    # in about 2 s. These are declared contract limits, passed through the arm command.
    parser.add_argument("--max-callbacks", type=int, default=None)
    parser.add_argument("--limit-seconds", type=int, default=None)
    args = parser.parse_args()
    state = args.output.parent / (args.output.name + ".STATE.txt")

    def note(text):
        line = time.strftime("%H:%M:%S ") + text
        print(line, flush=True)
        with state.open("a", encoding="utf-8") as stream:
            stream.write(line + "\n")

    argv = [sys.executable, "-B", str(RUNNER), "--mode", args.mode, "--output", str(args.output)]
    if args.mode == "live":
        argv += ["--port", str(args.port), "--session-id", args.session_id]
    child = subprocess.Popen(argv, stdin=subprocess.PIPE, stdout=subprocess.PIPE, text=True, bufsize=1)
    note("runner: " + child.stdout.readline().strip())

    def send(command):
        child.stdin.write(json.dumps(command) + "\n")
        child.stdin.flush()
        response = json.loads(child.stdout.readline())
        if not response.get("ok"):
            raise RuntimeError(json.dumps(response)[:2000])
        return response["result"]

    try:
        # The CE plugin reconnects to the runner's fresh bridge after a delay.
        for attempt in range(60):
            try:
                pre = send({"cmd": "preflight", "pid": args.pid, "attach": True})
                break
            except RuntimeError as error:
                if "is not connected" not in str(error) or attempt == 59:
                    raise
                if attempt % 10 == 0:
                    note("waiting for CE plugin session to connect")
                time.sleep(2.0)
        note("preflight ready=%s" % pre.get("ready"))
        if pre.get("ready") is not True:
            raise RuntimeError("preflight not ready: " + json.dumps(pre)[:2000])
        # Zero-HP frames (guard / perfect-guard ki damage) are skipped by the
        # observer, so allow several frames and their read budgets per window.
        limits = {key: value for key, value in (("max_callbacks", args.max_callbacks), ("seconds", args.limit_seconds))
                  if value is not None}
        armed = send({"cmd": "arm", "mode": "core_main", **({"limits": limits} if limits else {})})
        note("arm proof: " + json.dumps(armed.get("status", {}).get("global_breakpoints")))
        note("ARMED: capture window open; hit an enemy once now")
        if args.mode == "mock":
            for name in ("entry", "final", "hp_after"):  # core_main sites only
                send({"cmd": "mock_event", "name": name})
        deadline = time.time() + args.wait_seconds
        while True:
            status = send({"cmd": "status"})
            frames = status.get("frames", status.get("frame_count"))
            done = status.get("completed_frames") or status.get("hp_after_events") or status.get("capture_complete")
            if done or status.get("active") is False:
                note("hit observed: " + json.dumps({k: status.get(k) for k in ("active", "completed_frames", "hp_after_events", "capture_complete", "events")})[:600])
                break
            if time.time() > deadline:
                note("timeout without a completed hit")
                break
            # Multi-hit windows never complete on their own: <output>.STOP ends the window early, with the
            # normal export and observer cleanup below.
            stop_file = args.output.parent / (args.output.name + ".STOP")
            if stop_file.exists():
                note("stop requested: frames=%s" % frames)
                break
            time.sleep(1.0)
        exported = send({"cmd": "export", "file": "capture.json"})
        note("exported " + json.dumps(exported))
    finally:
        try:
            note("stop " + json.dumps(send({"cmd": "stop"}))[:600])
        except Exception as error:
            note("stop error " + str(error))
        child.stdin.close()
        child.wait(timeout=120)
        note("runner exit %s; disarmed" % child.returncode)


if __name__ == "__main__":
    main()
