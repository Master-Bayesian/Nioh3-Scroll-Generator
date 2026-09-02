from __future__ import annotations

"""Build stable offline R4 finalizer data from a verified runtime capture."""

import argparse
from pathlib import Path
import sys
from typing import Sequence


SCRIPT_DIR = Path(__file__).resolve().parent
PROJECT_ROOT = SCRIPT_DIR.parent
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.r4_finalizer_resource import build_r4_finalizer_resource


def parse_args(argv: Sequence[str] | None = None) -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Build a versioned, pointer-free offline resource from a verified "
            "Nioh 3 R4 finalizer runtime-table capture"
        )
    )
    parser.add_argument("--capture", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--source-locale", default="unknown")
    return parser.parse_args(argv)


def main(argv: Sequence[str] | None = None) -> int:
    args = parse_args(argv)
    output = build_r4_finalizer_resource(
        args.capture,
        args.output,
        source_locale=args.source_locale,
    )
    print(f"Built R4 finalizer resource: {output}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
