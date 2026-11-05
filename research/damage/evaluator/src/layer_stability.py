"""Equipment model check across every saved capture: captured aggregate - modelled items should be the same non-equipment
layer for all outfits of one day, whatever the armor, weapon and set state. argv: out.json [layers.json]"""
import glob
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from composed_model import ComposedModel, MAPS, load_layers  # noqa: E402
from outfit_model import Unknown  # noqa: E402

CACHE = "F:/Nioh3_r16_build/cache"
out = sys.argv[1]
current = None
if len(sys.argv) > 2:
    layers = load_layers(sys.argv[2])
    current = {0: {}, 1: {}}
    for name, layer in layers.items():
        if name in ("guardian:phoenix", "guardian:nekomata"):
            continue
        for f in (0, 1):
            for g, v in layer.get(f, {}).items():
                current[f][g] = current[f].get(g, 0) + v
paths = sorted(p.replace("\\", "/") for p in glob.glob("F:/Nioh3_r16_build/work/**/capture.json", recursive=True))
paths.append("F:/Nioh3_r16_build/deliverables/r16-damage-evaluator-v1-20261021/inputs/baseline-1016/capture.json")
rows, seen = [], set()


def total(entries):
    return {e["key_u16"]: (e["base_u32"] + e["duplicate_u16"]) & 0xFFFFFFFF for e in entries}


for path in paths:
    try:
        m = ComposedModel(CACHE, path)
    except Exception as e:  # captures of other shapes have no entry snapshot
        print("skip", path[-44:], type(e).__name__)
        continue
    form = m.mode
    want = total(m.captured_source[MAPS[form]])
    sig = (bytes(m.inv), tuple(sorted(want.items())))
    if sig in seen:
        continue
    seen.add(sig)
    try:
        got = total(m.items()[MAPS[form]])
    except (Unknown, Exception) as e:
        print("items failed", path[-44:], type(e).__name__, str(e)[:160])
        rows.append({"path": path, "failed": str(e)[:200]})
        continue
    layer = {g: want.get(g, 0) - got.get(g, 0) for g in set(want) | set(got) if want.get(g, 0) != got.get(g, 0)}
    rows.append({"path": path, "form": form, "attrs": m.raw_attrs, "layer": {hex(g): v for g, v in sorted(layer.items())}})
    if current is not None:
        diff = {hex(g): (layer.get(g, 0), current[form].get(g, 0)) for g in sorted(set(layer) | set(current[form]))
                if layer.get(g, 0) != current[form].get(g, 0)}
        print(path[len("F:/Nioh3_r16_build/"):], "form", form, "groups", len(want), "layer", len(layer), "diff vs 11-04", len(diff), list(diff.items())[:40], flush=True)
Path(out).write_text(json.dumps(rows, ensure_ascii=False, indent=0), encoding="utf-8")
