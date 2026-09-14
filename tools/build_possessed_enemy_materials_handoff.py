"""Retired guard for the superseded 2026-09-10 possessed-enemy package."""

from __future__ import annotations

import sys


MESSAGE = """This builder is retired because its research premise and package
layout were superseded by controlled PC v2.01 evidence. It intentionally cannot
rebuild or overwrite the historical 20260910_v2 package.

Use tools/build_possessed_enemy_assignment_handoff.py for the immutable
20260913_v1 GPT-6 Pro handoff.
"""


def main() -> int:
    sys.stderr.write(MESSAGE)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
