"""Checks of situations.py on the accepted 10-21 loadout (bank + scroll, observed 1114 plain).

1. "plain" with the default D cells added must still give the observed 1114.
2. "after_deflect" by state write must equal the accepted buff path (Evaluator(buff=1.1)), observed 1120 family.
3. Every single situation evaluates (no UNKNOWN) and is reported against plain.
"""
import json
import sys
from pathlib import Path

HERE = Path(__file__).resolve().parent
sys.path.insert(0, str(HERE / "src"))
from fast_eval import Evaluator  # noqa: E402
from regress import BANK, BASE, CACHE, IN  # noqa: E402
from regress_fast import load_inventory, pieces  # noqa: E402
from situations import NAMES, overrides  # noqa: E402

plist = pieces(BANK + ",0x870@15", load_inventory("bank-inv-scroll2"))
ev = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
               [IN / "grade_table_cells.json", IN / "default_d_cells.json"])
buffed = Evaluator(CACHE, BASE / "capture.json", BASE / "ENTRY_REQUEST.json", BASE / "RETAINED_STATIC_CELLS.json",
                   [IN / "grade_table_cells.json"], buff=1.1)
rows = {name: ev.evaluate(plist, overrides(ev, name))["damage"] for name in NAMES}
rows["back+ki_exhausted+status_poison"] = ev.evaluate(plist, overrides(ev, "back+ki_exhausted+status_poison"))["damage"]
rows["strong+low_stance"] = ev.evaluate(plist, overrides(ev, "strong+low_stance"))["damage"]
checks = {"plain_1114": rows["plain"] == 1114.0,
          "after_deflect_equals_buff_path": rows["after_deflect"] == buffed.evaluate(plist)["damage"],
          "no_unknown": all(v is not None for v in rows.values())}
print(json.dumps({"checks": checks, "damage": rows}, ensure_ascii=False, indent=1))
sys.exit(0 if all(checks.values()) else 1)
