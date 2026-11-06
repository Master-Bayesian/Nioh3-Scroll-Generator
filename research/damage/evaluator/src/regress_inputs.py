"""Every number the game showed for this hit, predicted from inputs alone by state_predict.py. argv: layers.json out.json"""
import json
import struct
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from state_predict import ITEM, Predictor, W  # noqa: E402

p = Predictor(sys.argv[1])
print("baseline replay", p.base_replay, flush=True)
live = Path(W + "live-1106/core2/attr_1b63eb628d0.bin").read_bytes()
item = lambda s: live[0x80 + s * ITEM:0x80 + (s + 1) * ITEM]
worn = [p.core(item(0)), p.core(item(1))]
print("cores", [(hex(c["key"]), c["attack"], c["defense"], {hex(g): v for g, v in c["groups"].items()}) for c in worn], flush=True)
H1 = "blind-1023/h1"
CASES = [("no armor", "blind-1023/base", None, None, 793), ("no armor again", "blind-1023/h2", None, None, 793), ("armored", H1, None, None, 884),
         ("second core", H1, None, worn, 1038), ("mistaken attributes", H1, [5, 5, 5, 36, 6, 5, 140], worn, 849),
         ("attributes a", H1, [37, 150, 27, 36, 80, 15, 140], worn, 1043), ("attributes c", H1, [37, 140, 27, 26, 80, 15, 160], worn, 1031),
         ("attributes b", H1, [37, 150, 37, 26, 80, 15, 140], worn, 1036)]
rows = []
for label, capture, attrs, cores, game in CASES:
    t0 = time.time()
    r = p.predict(capture, attrs, None, cores)
    r.pop("aggregate")
    rows.append({"label": label, "game": game, **r})
    print(label, "game", game, "predicted", r["damage"], "defense", r["defense"], "hp", r["hp"], "groups", r["groups"], f"{time.time() - t0:.1f}s", flush=True)
Path(sys.argv[2]).write_text(json.dumps(rows, ensure_ascii=False, indent=1), encoding="utf-8")
print("ALL MATCH" if all(r["damage"] == r["game"] for r in rows) else "MISMATCH")
