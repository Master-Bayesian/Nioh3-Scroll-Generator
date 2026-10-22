"""Best-known five-slot armor outfit for one captured hit: per-slot top-K start, then coordinate local search.

Every evaluated outfit is a full native evaluation by rank_outfits.py (STAT/D model + MAIN replay), so slot
interactions (weight curve, aggregation caps, set rewards) are included. This is a local search over the top-K
single pieces per slot, not a certified optimum over the whole backpack.
argv: <single RANKING.json> <out_dir> <K> -- <rank_outfits.py arguments without out_dir/--outfit>
"""
import json
import subprocess
import sys
from pathlib import Path

HERE = Path(__file__).parent
ranking, out, K = Path(sys.argv[1]), Path(sys.argv[2]), int(sys.argv[3])
rest = sys.argv[sys.argv.index("--") + 1:]
cache, capture, request, inventory, retained, *options = rest
singles = json.loads(ranking.read_bytes())["results"]
per_slot = {}
for x in singles:
    if x["damage"] is not None:
        per_slot.setdefault(x["pieces"][0]["slot"], []).append((x["damage"], x["pieces"][0]["F0_offset"]))
top = {s: [o for _, o in sorted(v, key=lambda t: -t[0])[:K]] for s, v in per_slot.items()}
seen = {}


def evaluate(outfits, round_name):
    todo = [o for o in outfits if tuple(sorted(o.items())) not in seen]
    if not todo:
        return
    specs = []
    for o in todo:
        name = "o_" + "_".join(o[s][2:] for s in sorted(o))
        specs += ["--outfit", name + "=" + ",".join(o[s] for s in sorted(o))]
    subprocess.run([sys.executable, str(HERE / "rank_outfits.py"), cache, capture, request, inventory,
                    str(out / round_name), retained, *options, *specs], check=True, capture_output=True, text=True)
    results = json.loads((out / round_name / "RANKING.json").read_bytes())["results"]
    for o, r in zip(todo, results):
        seen[tuple(sorted(o.items()))] = r
        print(json.dumps({"round": round_name, "outfit": o, "damage": r["damage"], "status": r["status"]}), flush=True)


best = {s: top[s][0] for s in top}
evaluate([best], "r0")
for step in range(1, 6):
    neighbours = [{**best, s: alt} for s in top for alt in top[s] if alt != best[s]]
    evaluate(neighbours, f"r{step}")
    cand = max((o for o in [best] + neighbours if seen[tuple(sorted(o.items()))]["damage"] is not None),
               key=lambda o: seen[tuple(sorted(o.items()))]["damage"])
    if seen[tuple(sorted(cand.items()))]["damage"] <= seen[tuple(sorted(best.items()))]["damage"]:
        break
    best = cand
result = {"best_known": best, "best_damage": seen[tuple(sorted(best.items()))]["damage"],
          "best_detail": seen[tuple(sorted(best.items()))], "evaluated": len(seen),
          "scope": f"coordinate local search over the top-{K} single pieces per slot; best-known, not certified optimal",
          "all": [{"outfit": dict(k), "damage": v["damage"]} for k, v in seen.items()]}
(out / "SEARCH.json").write_text(json.dumps(result, indent=1) + "\n")
print(json.dumps({"best": best, "damage": result["best_damage"], "evaluated": len(seen)}))
