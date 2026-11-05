"""Diff two ledger snapshots: signed change after - before. argv: before.json after.json"""
import json, sys
a = json.load(open(sys.argv[1], encoding="utf8")); b = json.load(open(sys.argv[2], encoding="utf8"))
print("source", a["source"], b["source"], "rows", len(a["rows"]), len(b["rows"]))
for form in (0, 1):
    x = {r["group"]: r for r in a["rows"] if r["form"] == form}
    y = {r["group"]: r for r in b["rows"] if r["form"] == form}
    print("== form", form, len(x), len(y))
    for g in sorted(set(x) | set(y)):
        p, q = x.get(g), y.get(g)
        vp, vq = p["live"] if p else None, q["live"] if q else None
        if vp != vq:
            print(g, (p or q)["name"], vp, "->", vq, "change", (vq or 0) - (vp or 0))
