"""Parity of the in-process evaluator (src/fast_eval.py) with every accepted observation in regress.py.

Each case is evaluated twice: a fresh replay, then the same candidate again, which must be answered by the
read-set memo with the same damage. The last case of each evaluator is also run through confirm() (the full v1
acceptance path with the initial-score ablation). Run through the repository wrapper, like regress.py.
"""
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "src"))
from fast_eval import Evaluator  # noqa: E402
from regress import BASE, CACHE, CASES, IN  # noqa: E402

OUT = Path("F:/Nioh3_r16_build/work") / time.strftime("regress-fast-%Y%m%d-%H%M%S")


def load_inventory(name):
    catalog = json.loads((IN / name / "INVENTORY.json").read_bytes())
    records = next(v for v in catalog.values() if isinstance(v, list) and len(v) > 100)
    return {r["F0_offset"]: r for r in records if r["F0_offset"] is not None}, (IN / name / "INVENTORY_F0.bin").read_bytes()


def pieces(spec, inventory):
    """Outfit spec as rank_outfits.py reads it: "F0off" (armor slot from the catalog) or "F0off@slot"."""
    records, f0 = inventory
    out = []
    for o in spec.split(","):
        off, _, slot = o.partition("@")
        off = int(off, 16)
        out.append((f0[off:off + 0xE8], int(slot) if slot else records[off]["physical_armor_slot"]))
    return out


def main():
    OUT.mkdir(parents=True)
    rows, evaluators = [], {}
    started = time.time()
    for name, inventory, buff, spec, observed, how in [("baseline_714", None, None, "", 714, "natural")] + CASES:
        if buff not in evaluators:
            evaluators[buff] = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                                         [IN / "grade_table_cells.json"], buff=buff)
        ev = evaluators[buff]
        p = pieces(spec, load_inventory(inventory)) if spec else []
        first, again = ev.evaluate(p), ev.evaluate(p)
        rows.append({"case": name, "observed": observed, "fresh": first["damage"], "memo": again["damage"],
                     "memo_hit": again["memo"], "match": first["damage"] == observed == again["damage"] and again["memo"]})
        print(json.dumps(rows[-1], ensure_ascii=False), flush=True)
    for buff, ev in evaluators.items():
        case = [c for c in CASES if c[2] == buff][-1]
        confirmed = ev.confirm(pieces(case[3], load_inventory(case[1])))
        rows.append({"case": case[0] + ":confirm", "observed": case[4], "fresh": confirmed["damage"],
                     "match": confirmed["damage"] == case[4] and confirmed["accepted"]})
        print(json.dumps(rows[-1], ensure_ascii=False), flush=True)
    stats = {str(b): ev.stats for b, ev in evaluators.items()}
    summary = {"all_match": all(r["match"] for r in rows), "cases": rows, "stats": stats, "seconds": time.time() - started}
    (OUT / "REGRESSION_FAST.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps({"all_match": summary["all_match"], "stats": stats, "seconds": summary["seconds"], "output": str(OUT)}))
    sys.exit(0 if summary["all_match"] else 1)


if __name__ == "__main__":
    main()
