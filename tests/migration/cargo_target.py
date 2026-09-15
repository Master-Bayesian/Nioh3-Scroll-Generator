"""One resolved Cargo target directory for the migration parity gates.

`CARGO_TARGET_DIR` always wins, so an operator or CI can point every gate at a
volume with room and share one cache. Otherwise the target lives under the
platform temp directory: a parity gate must never implicitly write a fresh
target tree into the checkout, whose volume can be full (the disk-full failure
this helper exists to prevent).

Keep this the only resolver. A gate that hardcodes its own target path is the
bug `tests/migration/test_cargo_target_defaults.py` refuses.
"""
from __future__ import annotations

import os
import tempfile
from pathlib import Path


def resolved_cargo_target_dir(name: str = "migration") -> str:
    """The cargo target directory one gate must build into."""

    configured = os.environ.get("CARGO_TARGET_DIR", "").strip()
    if configured:
        return str(Path(configured))
    return str(Path(tempfile.gettempdir()) / f"nioh3-{name}-target")
