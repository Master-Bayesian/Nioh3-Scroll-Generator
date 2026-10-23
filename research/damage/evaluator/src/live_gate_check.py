"""Check a live-captured hit situation: replay its filled request, then list the effect fields in its formula.

For a hit captured in the situation itself (martial art, airborne, element, ailing target), the unprobed replay must
reproduce the game's damage, and with the effect probe the situation's conditional fields must be formula leaves.
argv: <capture.json> <ENTRY_REQUEST.json> <FILLED_CELLS.json> <out_dir> [--expect D+0x550 ...]
"""
import argparse
import json
import struct
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE))
sys.path.insert(0, str(HERE.parent))
from formula_trace import f32, trace  # noqa: E402
from gate_scan import formula_fields  # noqa: E402
from zone_table import NAMES, effect_names, group_fields  # noqa: E402


def main():
    p = argparse.ArgumentParser()
    p.add_argument("capture", type=Path)
    p.add_argument("request", type=Path)
    p.add_argument("filled", type=Path)
    p.add_argument("out", type=Path)
    p.add_argument("--expect", nargs="*", default=[])
    p.add_argument("--exclude", nargs="*", default=["0x104", "0x530", "0x714"], help="D offsets never probed")
    args = p.parse_args()
    from fast_eval import Evaluator
    from regress_fast import CACHE
    ev = Evaluator(CACHE, args.capture, args.request, args.filled, observe_only=True)
    m = ev.model
    fields, _, _ = group_fields(m)
    names = effect_names(m, NAMES)
    by_field = {f"{o}+{off:#x}": sorted({n for g in gs for n in names.get(g, [])})[:4] for (o, off), gs in fields.items()}
    plain = ev.evaluate([])
    sh0, root0, _, _ = trace(ev, [])
    base = formula_fields(sh0, root0) or {}
    probe = {}
    for row in m.F.catalog()["recipes"].values():
        for op in row.get("operations", []):
            if str(op.get("equation", "")).startswith("D_f32") and op.get("width") == 4:
                a = m.d_addr + int(op["destination"], 16)
                if all((ev._template_byte(a + i) or (0, 0))[0] == 0 for i in range(4)):
                    probe[a] = struct.pack("<f", 0.01)
    for bad in args.exclude:
        probe.pop(m.d_addr + int(bad, 16), None)
    sh, root, _, _ = trace(ev, [], probe)
    got = formula_fields(sh, root)
    out = {"damage": plain["damage"], "status": plain["status"], "reason": plain.get("reason"),
           "probed_damage": f32(sh.nodes[root]["value_u"]) if root is not None else None,
           "fields": {f: by_field.get(f, []) for f in sorted(base)},
           "probed_fields": {f: by_field.get(f, []) for f in sorted(got or {})},
           "expected": {e: bool(got and e in got) for e in args.expect}}
    args.out.mkdir(parents=True, exist_ok=True)
    (args.out / "LIVE_GATE_CHECK.json").write_text(json.dumps(out, indent=1, ensure_ascii=False) + "\n", encoding="utf-8")
    print(json.dumps({k: out[k] for k in ("damage", "status", "probed_damage", "expected")}, ensure_ascii=False))


if __name__ == "__main__":
    main()
