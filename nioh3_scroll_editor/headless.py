"""Minimal JSON command surface for backend-contract smoke tests."""

from __future__ import annotations

import argparse
from dataclasses import asdict
import json
import sys

from .core_services import CandidateApplicationService, CoreServiceError
from .models import CandidateRecordStage, ScrollCandidate


def handle_command(payload: dict[str, object]) -> dict[str, object]:
    service = CandidateApplicationService()
    command = payload.get("command")
    if command == "handshake":
        return {
            "ok": True,
            "protocol": "nioh3-core-smoke/v1",
            "context": service.context.to_payload(),
        }
    if command == "candidate_preflight":
        record = bytes.fromhex(str(payload["record_hex"]))
        installation_hex = payload.get("installation_record_hex")
        candidate = ScrollCandidate.from_record(
            record,
            playthrough=(
                int(payload["playthrough"])
                if payload.get("playthrough") is not None
                else None
            ),
            record_stage=CandidateRecordStage(
                str(payload.get("record_stage", "final_record"))
            ),
        )
        if installation_hex:
            from dataclasses import replace

            candidate = replace(
                candidate,
                installation_record=bytes.fromhex(str(installation_hex)),
            )
        preview = service.preview(candidate)
        result = asdict(preview)
        result["record_stage"] = preview.record_stage.value
        return {"ok": True, "preview": result}
    raise ValueError(f"unsupported command: {command!r}")


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--once", help="one JSON command; otherwise read JSON lines")
    args = parser.parse_args()
    inputs = (args.once,) if args.once is not None else sys.stdin
    for raw in inputs:
        try:
            payload = json.loads(raw)
            if not isinstance(payload, dict):
                raise ValueError("command root must be an object")
            result = handle_command(payload)
        except CoreServiceError as error:
            result = {"ok": False, "error": error.to_payload()}
        except Exception as error:
            result = {
                "ok": False,
                "error": {"code": "INVALID_COMMAND", "message": str(error)},
            }
        print(json.dumps(result, ensure_ascii=False, separators=(",", ":")))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
