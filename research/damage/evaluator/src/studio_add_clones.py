"""Add exact clones of chosen owned equipment through Nioh3Studio's live equipment addition (same id/level/plus/rarity/seed).

Each item is prepared first; it is executed only if the preview's effects and values equal the original's.
argv: <PICKED.json> <save_path> [--execute]
"""
import json
import sys
import uuid
from pathlib import Path

sys.path.insert(0, str(Path(__file__).parent))
from studio_client import Worker  # noqa: E402

picked = json.loads(Path(sys.argv[1]).read_text(encoding="utf-8"))
save_path, execute = sys.argv[2], "--execute" in sys.argv
log = Path(sys.argv[1]).with_name("ADD_LOG.jsonl")
w = Worker("runtime")
try:
    w.call("handshake")
    for item in picked:
        op = str(uuid.uuid4())
        params = {"operation_id": op, "item_id": item["item_id"], "level": item["level"], "plus": item["plus"],
                  "rarity": item["rarity"], "seed": item["seed"], "save_path": save_path}
        prep = w.call("runtime.equipment_add_prepare", params)
        state = ((prep.get("result") or {}).get("result") or prep.get("result") or {}).get("equipment_add") or {}
        preview = state.get("preview") or {}
        got = [(e["effect_id"], e["value"]) for e in preview.get("effects", []) if e["effect_id"] != 0xFFFFFFFF]
        want = [tuple(x) for x in item["effects"]]
        same = (not want or got == want) and preview.get("level") == item["level"] and preview.get("plus") == item["plus"]
        row = {"name": item["name"], "operation_id": op, "prepare_ok": prep.get("ok"), "state": state.get("state"),
               "plan_digest": state.get("plan_digest"), "preview_effects": got, "original_effects": want, "identical": same,
               "error": prep.get("error") or state.get("error")}
        if execute and same and state.get("plan_digest"):
            done = w.call("runtime.equipment_add_execute", {"operation_id": op, "plan_digest": state["plan_digest"], "confirmed": True})
            final = ((done.get("result") or {}).get("result") or done.get("result") or {}).get("equipment_add") or {}
            row.update(executed=True, execute_ok=done.get("ok"), final_state=final.get("state"),
                       slot_index=final.get("slot_index"), execute_error=done.get("error") or final.get("error"))
        elif state.get("plan_digest"):
            w.call("runtime.equipment_add_cancel", {"operation_id": op})
            row["cancelled"] = True
        print(json.dumps(row, ensure_ascii=False), flush=True)
        with log.open("a", encoding="utf-8") as stream:
            stream.write(json.dumps(row, ensure_ascii=False) + "\n")
finally:
    w.close()

