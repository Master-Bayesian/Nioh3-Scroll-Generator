"""Name every non-equipment contribution of the 11-04 aggregate and write SOURCE_LAYERS.json.

Inputs are saved live reads, no game needed. A layer maps form (0 samurai, 1 ninja) -> {group: value}. Layers that
a loadout can swap (guardian per style, soul core, clan blessing, skills, title blessings) stay separate; progress that
does not change with a loadout is the rest. argv: data_dir out.json
"""
import collections, json, sys
from pathlib import Path

data, out = Path(sys.argv[1]), Path(sys.argv[2])
led = json.loads((data / "LEDGER.json").read_bytes())
guard = json.loads((data / "LEDGER_guard.json").read_bytes())
nocore = json.loads((data / "LEDGER_nocore.json").read_bytes())  # the guardian swap was read with the core removed
talents = json.loads((data / "TALENTS.json").read_bytes())
layers = collections.defaultdict(lambda: {0: {}, 1: {}})

for r in led["rows"]:
    for value, label in r.get("parts") or []:
        name = "clan_blessing" if label == "slot6340" else "title_blessings" if label == "恩惠" else "skills"
        at = layers[name][r["form"]]
        at[r["group"]] = at.get(r["group"], 0) + value
GUARDIANS = {"kusanagi": (0, ("0xaf2c", "0xb2d3", "0xfdde", "0x62c5", "0x9528")),
             "tora_gozen": (1, ("0x2e35", "0x4d1c", "0x9554", "0xf8c9", "0x1514"))}
live = {(r["form"], r["group"]): r for r in led["rows"]}
for name, (form, groups) in GUARDIANS.items():
    for g in groups:
        r = live[(form, g)]
        layers["guardian:" + name][form][g] = r["live"] - r["known"] - (10 if g == "0xb2d3" else 0)  # 10 = the worn item
# Swapped-in guardians: the change of the aggregate plus the removed guardian.
removed = {f: {g: v for n, l in layers.items() if n.startswith("guardian:") for g, v in l[f].items()} for f in (0, 1)}
for name, form in (("phoenix", 0), ("nekomata", 1)):
    now = {r["group"]: r["live"] for r in guard["rows"] if r["form"] == form}
    before = {r["group"]: r["live"] for r in nocore["rows"] if r["form"] == form}
    for g in set(now) | set(before):
        change = now.get(g, 0) - before.get(g, 0) + removed[form].get(g, 0)
        # Form 0 of the no-core read was still stale, so the core's own groups are not part of the guardian.
        if change and g not in ("0x9ae9", "0x3a1f", "0xf63d"):
            layers["guardian:" + name][form][g] = change
for form in (0, 1):
    layers["soul_core:sand_hag"][form].update({"0x9ae9": 78, "0x3a1f": 11, "0xf63d": 50, "0x5dc3": 0})
    for t in talents:
        # Region upgrades (source7c8) are filtered by the current region; none reached this aggregate.
        if t.get("current_value") and t["family"] == "source7c0":
            g = t["group"]
            at = layers["elixir_upgrades"][form]
            at[g] = at.get(g, 0) + t["current_value"]
    layers["area_completion"][form].update({"0x6a7c": 88, "0x4bb6": 88, "0x3b0e": 178})
    layers["unnamed_progress"][form].update({"0x52d": 36, "0xf919": 35})  # equal in both forms, no text, source open
Path(out).write_text(json.dumps({k: {str(f): v for f, v in l.items()} for k, l in sorted(layers.items())}, ensure_ascii=False, indent=1), encoding="utf-8")
for k, l in sorted(layers.items()):
    print(k, {f: len(v) for f, v in l.items()}, l[0] if k.startswith(("guardian", "region", "elixir", "area")) else "", l[1] if k.startswith(("guardian", "region", "elixir")) else "")
