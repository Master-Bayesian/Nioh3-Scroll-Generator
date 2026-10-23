"""Situation-aware armor optimizer on the 10-16 baseline hit (mid-stance first light katana attack, training samurai).

Objective: sum over scenarios of weight * damage, where each scenario is a "+"-joined list of situations from
situations.py (e.g. "plain", "back+ki_exhausted", "after_deflect+status_poison"). Every damage is a full native
MAIN replay (fast_eval.Evaluator), so set rewards, weight curve and pool dilution are exact for this hit.

Search: the current loadout (bank: weapon, accessories, scroll, armor) is the start. Stage 1 swaps every backpack
armor piece alone into the start and scores it. Stage 2 runs coordinate ascent over the top-K pieces per slot,
from the start and from the best single per slot. The result is best-known, not a certified optimum.
argv: <out_dir> --scenario NAME=WEIGHT ... [--bank dir --bank-spec spec --backpack dir --top K]
"""
import argparse
import json
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from situations import NAMES, overrides  # noqa: E402

ARMOR_SLOTS = (4, 5, 6, 7, 8)
BANK_SPEC = "0x0@0,0xf0@4,0x1e0@5,0x2d0@6,0x3c0@7,0x4b0@8,0x5a0@9,0x690@10,0x870@15"


def main():
    p = argparse.ArgumentParser()
    p.add_argument("out", type=Path)
    p.add_argument("--scenario", action="append", required=True, help="NAME=WEIGHT; situations: " + ", ".join(NAMES))
    p.add_argument("--bank", default="bank-inv-scroll2")
    p.add_argument("--bank-spec", default=BANK_SPEC)
    p.add_argument("--backpack", default="live-inventory-1021")
    p.add_argument("--top", type=int, default=6)
    p.add_argument("--form", choices=("samurai", "ninja"), default="samurai", help="form of the captured hit's loadout")
    p.add_argument("--names", type=Path, default=None)
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import BASE, CACHE, IN, load_inventory
    started = time.time()
    scenarios = []
    for s in args.scenario:
        name, _, weight = s.rpartition("=")
        scenarios.append((name, float(weight)))
    ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json", IN / "default_d_cells.json"])
    states = {name: overrides(ev, name) for name, _ in scenarios}

    bank_records, bank_f0 = load_inventory(args.bank)
    start = {}
    for o in args.bank_spec.split(","):
        off, _, slot = o.partition("@")
        start[int(slot)] = ("bank", int(off, 16), bank_f0[int(off, 16):int(off, 16) + 0xE8], bank_records[int(off, 16)]["name"])
    pack_records, pack_f0 = load_inventory(args.backpack)
    # Samurai and ninja forms wear separate armor; only pieces of the captured hit's form are candidates.
    pool = {s: [("backpack", off, pack_f0[off:off + 0xE8], r.get("name")) for off, r in sorted(pack_records.items())
                if r.get("kind") == "armor" and r.get("physical_armor_slot") == s and r.get(args.form) is True]
            for s in ARMOR_SLOTS}
    for s in ARMOR_SLOTS:
        if s in start:
            pool[s].insert(0, start[s])
    cache = {}

    def score(loadout):
        key = tuple(sorted((s, src, off) for s, (src, off, _, _) in loadout.items()))
        if key not in cache:
            pieces = [(data, s) for s, (_, _, data, _) in sorted(loadout.items())]
            per, total = {}, 0.0
            for name, weight in scenarios:
                r = ev.evaluate(pieces, states[name])
                if r["damage"] is None:
                    per, total = {"UNKNOWN": r.get("reason")}, None
                    break
                per[name] = r["damage"]
                total += weight * r["damage"]
            cache[key] = (total, per)
        return cache[key]

    def describe(loadout):
        return {str(s): {"source": src, "F0_offset": hex(off), "name": name} for s, (src, off, _, name) in sorted(loadout.items())}

    base_total, base_per = score(start)
    print(json.dumps({"start": base_per, "objective": base_total}, ensure_ascii=False), flush=True)
    singles = {s: [] for s in ARMOR_SLOTS}
    for s in ARMOR_SLOTS:
        for piece in pool[s]:
            total, per = score({**start, s: piece})
            if total is not None:
                singles[s].append((total, piece))
        singles[s].sort(key=lambda t: -t[0])
        print(json.dumps({"slot": s, "candidates": len(pool[s]), "scored": len(singles[s]),
                          "best": [round(t, 1) for t, _ in singles[s][:3]], "seconds": round(time.time() - started)}), flush=True)
    top = {s: [piece for _, piece in singles[s][:args.top]] for s in ARMOR_SLOTS}
    best_overall = (base_total, start)
    for seed in (start, {**start, **{s: top[s][0] for s in ARMOR_SLOTS if top[s]}}):
        current, current_total = dict(seed), score(seed)[0]
        while True:
            moves = [{**current, s: alt} for s in ARMOR_SLOTS for alt in top[s] if alt[:2] != current[s][:2]]
            scored = [(score(m)[0], m) for m in moves]
            scored = [(t, m) for t, m in scored if t is not None]
            if not scored or max(t for t, _ in scored) <= current_total:
                break
            current_total, current = max(scored, key=lambda t: t[0])
        if current_total > best_overall[0]:
            best_overall = (current_total, current)
    total, best = best_overall
    result = {"scenarios": dict(scenarios), "start": {"loadout": describe(start), "objective": base_total, "damage": base_per},
              "best": {"loadout": describe(best), "objective": total, "damage": score(best)[1]},
              "evaluated_loadouts": len(cache), "replays": ev.stats, "seconds": time.time() - started,
              "scope": f"one hit (10-16 baseline action and target); coordinate ascent over the top-{args.top} single "
                       "swaps per armor slot; best-known, not certified optimal",
              "top_singles": {str(s): [{"objective": t, "name": piece[3], "F0_offset": hex(piece[1]), "source": piece[0]}
                                       for t, piece in singles[s][:args.top]] for s in ARMOR_SLOTS}}
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "OPTIMIZE.json").write_text(json.dumps(result, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({"start": base_per, "best": result["best"]["damage"], "objective": [base_total, total],
                      "seconds": round(result["seconds"])}, ensure_ascii=False))


if __name__ == "__main__":
    main()
