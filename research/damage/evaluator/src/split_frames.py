"""Split a multi-hit v5 capture into single-hit captures (frame renumbered to 1, only its events/snapshots kept).

argv: <capture.json> <out_dir> <frame_id> [<frame_id> ...]
"""
import copy
import json
import sys
from pathlib import Path

capture = json.loads(Path(sys.argv[1]).read_bytes())
out = Path(sys.argv[2])
raw = capture["raw_capture"]
for fid in (int(x) for x in sys.argv[3:]):
    frame = copy.deepcopy(next(f for f in raw["frames"] if f["id"] == fid))
    events = [copy.deepcopy(e) for e in raw["events"] if e.get("frame_id") == fid]
    ids = {e["snapshot_id"] for e in events}
    snaps = [copy.deepcopy(s) for s in raw["snapshots"] if s["id"] in ids]
    frame["id"] = 1
    for e in events:
        e["frame_id"] = 1
    single = {k: v for k, v in capture.items() if k != "raw_capture"}
    single["raw_capture"] = {k: v for k, v in raw.items() if k not in ("frames", "events", "snapshots")}
    single["raw_capture"].update(frames=[frame], events=events, snapshots=snaps)
    single["split_from"] = {"capture": str(sys.argv[1]), "original_frame_id": fid}
    target = out / f"frame{fid:02d}"
    target.mkdir(parents=True, exist_ok=True)
    (target / "capture.json").write_text(json.dumps(single))
    print(fid, "->", target, len(events), "events", len(snaps), "snapshots")
