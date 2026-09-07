"""Write a stable inventory of every unittest ID collected by the release runner."""

from __future__ import annotations

import argparse
from datetime import datetime, timezone
import json
from pathlib import Path
import platform
import sys
import unittest


def iter_test_ids(suite: unittest.TestSuite):
    for item in suite:
        if isinstance(item, unittest.TestSuite):
            yield from iter_test_ids(item)
        else:
            yield item.id()


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    suite = unittest.defaultTestLoader.discover(".")
    test_ids = tuple(sorted(iter_test_ids(suite)))
    if len(test_ids) != len(set(test_ids)):
        raise RuntimeError("the unittest runner collected duplicate test IDs")
    payload = {
        "schema": "nioh3-test-inventory/v1",
        "generated_at_utc": datetime.now(timezone.utc).isoformat(),
        "runner": "python -m unittest discover -v",
        "python": sys.version,
        "platform": platform.platform(),
        "test_count": len(test_ids),
        "test_ids": list(test_ids),
        "hardware_skip_policy": (
            "Tests requiring a physical CUDA/D3D11 device call skipTest with an "
            "explicit reason; strict-policy fault injection remains hardware-independent."
        ),
    }
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_text(
        json.dumps(payload, indent=2, ensure_ascii=False) + "\n",
        encoding="utf-8",
    )
    print(f"collected {len(test_ids)} unique unittest IDs -> {args.output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
