"""Freeze the search winners with the readings a menu can confirm. argv: layers.json out.json"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from state_predict import ITEM, Predictor, W  # noqa: E402

p = Predictor(sys.argv[1])
live = Path(W + "live-1106/core2/attr_1b63eb628d0.bin").read_bytes()
worn = [p.core(live[0x80 + s * ITEM:0x80 + (s + 1) * ITEM]) for s in range(2)]
rows = []
for attrs in ([37, 150, 27, 26, 80, 15, 150], [5, 200, 15, 75, 5, 35, 150], [5, 45, 15, 200, 5, 60, 155]):
    r = p.predict("blind-1023/h1", attrs, None, worn)
    r.pop("aggregate")
    rows.append({"attributes": attrs, **r})
    print(rows[-1], flush=True)
Path(sys.argv[2]).write_text(json.dumps(rows, indent=1), encoding="utf-8")
