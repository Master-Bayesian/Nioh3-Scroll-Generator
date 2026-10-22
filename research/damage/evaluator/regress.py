"""Offline regression of every accepted natural observation (Nioh 3 PC 2.0.2.0).

Re-runs each case from item bytes and the frozen 10-16 baseline through outfit_model + native MAIN replay, and
compares with the in-game damage the owner observed. Run through the repository wrapper:
  $env:NIOH3_BUILD_ROOT='F:/Nioh3_r16_build'
  F:/Nioh3_ScrollEditor/tools/run_python_tests.ps1 -MinimumFreeGiB 0 -ScriptPath <this file>
Outputs go to F:/Nioh3_r16_build/work/regress-<time>/ (never into this package).
"""
import json
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
SRC, IN = HERE / "src", HERE / "inputs"
OUT = Path("F:/Nioh3_r16_build/work") / time.strftime("regress-%Y%m%d-%H%M%S")
CACHE = "F:/Nioh3_r16_build/cache"
BASE = IN / "baseline-1016"
OUTFIT_1021 = "0xc30,0xe10,0xf00,0x11d0,0x12c0,0x13b0@0"
BANK = "0x0@0,0xf0@4,0x1e0@5,0x2d0@6,0x3c0@7,0x4b0@8,0x5a0@9,0x690@10"

# (case, inventory dir, buff, outfit spec, observed damage, how it was observed)
CASES = [
    ("helmet_720", "inventory-1020", None, "0x1ffe0", 720, "10-16 natural, after the hit (D/7B8 later derived)"),
    ("helmet_deflect_793", "inventory-1020", 1.1, "0x1ffe0", 793, "10-16 natural, after the hit"),
    ("outfit_bizen_1018", "live-inventory-1021", None, OUTFIT_1021, 1018, "10-21 blind"),
    ("outfit_bizen_deflect_1120", "live-inventory-1021", 1.1, OUTFIT_1021, 1120, "10-21 blind"),
    ("accessories_1018", "bank-inv-h4", None, BANK, 1018, "10-21 blind (叶二 + 八咫镜)"),
    ("scroll_1097", "bank-inv-h4", None, BANK + ",0x870@15", 1097, "10-21 reproduction (hit before slot 15 was modelled)"),
    ("scroll_copy_1114", "bank-inv-scroll2", None, BANK + ",0x870@15", 1114, "10-21 blind"),
]


def replay_baseline():
    run = OUT / "baseline_714.run"
    subprocess.run([sys.executable, str(SRC / "demand_fill_loop.py"), "--capture", str(BASE / "capture.json"), "--cache-dir", CACHE,
                    "--output-dir", str(run), "--offline", "--iterations", "2", "--resume-request", str(BASE / "ENTRY_REQUEST.json")],
                   capture_output=True, text=True)
    last = json.loads((run / "LOOP.jsonl").read_text().splitlines()[-1])
    return last.get("damage") if last.get("normal_return") else None


def main():
    OUT.mkdir(parents=True)
    rows = [{"case": "baseline_714", "observed": 714, "predicted": replay_baseline(), "how": "10-16 natural and 10-21 natural"}]
    groups = {}
    for case in CASES:
        groups.setdefault((case[1], case[2]), []).append(case)
    for (inventory, buff), cases in groups.items():
        tag = f"{inventory}-{'buff' if buff else 'plain'}"
        args = [sys.executable, str(SRC / "rank_outfits.py"), CACHE, str(BASE / "capture.json"), str(BASE / "ENTRY_REQUEST.json"),
                str(IN / inventory), str(OUT / tag), str(BASE / "RETAINED_STATIC_CELLS.json"),
                "--extra-cells", str(IN / "grade_table_cells.json"), "--names", str(IN / "item_names.json"), "--jobs", "4"]
        if buff:
            args += ["--buff", str(buff)]
        for name, _, _, spec, _, _ in cases:
            args += ["--outfit", f"{name}={spec}"]
        done = subprocess.run(args, capture_output=True, text=True)
        if done.returncode:
            print(done.stderr[-2000:])
        results = {r["tag"]: r for r in json.loads((OUT / tag / "RANKING.json").read_bytes())["results"]}
        for name, _, _, _, observed, how in cases:
            rows.append({"case": name, "observed": observed, "predicted": results.get(name, {}).get("damage"), "how": how})
    for row in rows:
        row["match"] = row["predicted"] == row["observed"]
        print(json.dumps(row, ensure_ascii=False))
    summary = {"all_match": all(r["match"] for r in rows), "cases": rows, "output": str(OUT)}
    (OUT / "REGRESSION.json").write_text(json.dumps(summary, ensure_ascii=False, indent=1), encoding="utf-8")
    print(json.dumps({"all_match": summary["all_match"], "output": str(OUT)}))
    sys.exit(0 if summary["all_match"] else 1)


if __name__ == "__main__":
    main()
