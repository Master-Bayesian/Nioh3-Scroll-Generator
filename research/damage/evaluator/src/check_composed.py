"""Offline acceptance of the composed aggregate on one 11-04 capture.

1. equipment + named layers == captured aggregate, both forms, every group.
2. D rebuilt from the composed aggregate == captured D on every effect word.
3. Removing or swapping one source: predicted damage of the captured spell hit.
argv: layers.json capture.json request.json empty_retained.json out.json
"""
import json
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
from composed_model import ComposedModel, load_layers  # noqa: E402
from fast_eval import Evaluator  # noqa: E402

CACHE = "F:/Nioh3_r16_build/cache"
layers_path, capture, request, retained, out = sys.argv[1:6]
layers = load_layers(layers_path)
WORN = {0: "guardian:kusanagi", 1: "guardian:tora_gozen"}
COMMON = ["title_blessings", "skills", "clan_blessing", "soul_core:sand_hag", "elixir_upgrades", "area_completion", "unnamed_progress"]
report = {"residual": {}}


def total(rows):
    return {e["key_u16"]: (e["base_u32"] + e["duplicate_u16"]) & 0xFFFFFFFF for e in rows}


for form in (0, 1):
    m = ComposedModel(CACHE, capture, form)
    source = m.compose(layers, COMMON + [WORN[form]])
    key = {0: "map_35d0", 1: "map_4728"}[form]
    want, got = total(m.captured_source[key]), total(source[key])
    bad = {hex(g): (want.get(g, 0), got.get(g, 0)) for g in sorted(set(want) | set(got)) if want.get(g, 0) != got.get(g, 0)}
    report["residual"][form] = {"groups": len(want), "wrong": bad}
    print("form", form, "groups", len(want), "wrong", bad)

m = ComposedModel(CACHE, capture)
form = m.mode
base_names = COMMON + [WORN[form]]
stat, d = m.materialize(m.compose(layers, base_names))
effect = [at for at in range(0, 0xB00, 4) if d[at:at + 4] != m.d0[at:at + 4]]
wrong = [hex(at) for at in effect if d[at:at + 4] != m.d[at:at + 4]]
print("D effect words", len(effect), "wrong", wrong)
report["d"] = {"effect_words": len(effect), "wrong": wrong}

ev = Evaluator(CACHE, capture, request, retained, [], observe_only=True)


def writes(stat2, d2):
    """Words that differ from the composed baseline; the replay keeps every captured word otherwise."""
    changed = {}
    for addr, new, old, size in ((m.stat_addr, stat2, stat, 0x7A0), (m.d_addr, d2, d, 0xB00)):
        for at in range(0, size, 4):
            if at not in (8, 12) and new[at:at + 4] != old[at:at + 4]:
                changed[addr + at] = new[at:at + 4]
    return changed


first = ev.evaluate([])
print("captured replay", first["damage"], first["status"], first.get("reason"))
variants = {"baseline": base_names}
for drop in ("title_blessings", "skills", "clan_blessing", "soul_core:sand_hag", WORN[form], "unnamed_progress", "elixir_upgrades"):
    variants["without " + drop] = [n for n in base_names if n != drop]
variants["guardian nekomata"] = [n for n in base_names if n != WORN[form]] + ["guardian:nekomata"]
report["damage"] = {"captured": first["damage"]}
for name, names in variants.items():
    s2, d2 = m.materialize(m.compose(layers, names))
    w = writes(s2, d2)
    r = ev.evaluate([], w)
    report["damage"][name] = r["damage"]
    print(name, r["damage"], r["status"], "words", len(w))
for label, delta in (("no debuff-count skill", {0xF1A8: -10}), ("ninjutsu +10%", {0x2A1F: 100})):
    s2, d2 = m.materialize(m.compose(layers, base_names, delta=delta))
    r = ev.evaluate([], writes(s2, d2))
    report["damage"][label] = r["damage"]
    print(label, r["damage"], r["status"])
Path(out).write_text(json.dumps(report, ensure_ascii=False, indent=1), encoding="utf-8")
