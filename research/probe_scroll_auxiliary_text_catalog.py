from __future__ import annotations

import argparse
import json
import sys
from dataclasses import asdict
from datetime import datetime, timezone
from pathlib import Path

RESEARCH_DIR = Path(__file__).resolve().parent
if str(RESEARCH_DIR) not in sys.path:
    sys.path.insert(0, str(RESEARCH_DIR))

from dump_effect_catalog_current_locale import (
    CURRENT_LANGUAGE_POOL_RADIUS,
    ProcessReader,
    TextEntry,
    _candidate_regions,
    _plausible_text,
    locate_localization_pool,
    normalize_locale,
    validate_version,
)


def iter_text_entries_from_block(block: bytes, base_address: int):
    """Yield plausible native localization entries from one in-memory block.

    Native entries use ``u32 text_id, u32 UTF-16 code-unit count, text[]``.
    The scan is intentionally limited to the already validated current-language
    pool; it is not a whole-process heuristic.
    """

    for offset in range(0, max(0, len(block) - 10), 2):
        code_units = int.from_bytes(block[offset + 4 : offset + 8], "little")
        if not 2 <= code_units <= 512:
            continue
        text_end = offset + 8 + code_units * 2
        if text_end > len(block) or block[text_end - 2 : text_end] != b"\x00\x00":
            continue
        try:
            text = block[offset + 8 : text_end - 2].decode("utf-16-le")
        except UnicodeDecodeError:
            continue
        if not _plausible_text(text):
            continue
        yield TextEntry(
            text_id=int.from_bytes(block[offset : offset + 4], "little"),
            address=base_address + offset,
            code_units_including_null=code_units,
            text=text,
        )


def match_entries(
    entries: list[TextEntry],
    terms: list[str],
    *,
    exact: bool,
) -> dict[str, list[TextEntry]]:
    matches: dict[str, list[TextEntry]] = {}
    for term in terms:
        if exact:
            selected = [entry for entry in entries if entry.text == term]
        else:
            selected = [entry for entry in entries if term in entry.text]
        matches[term] = selected
    return matches


def match_text_ids(
    entries: list[TextEntry],
    text_ids: list[int],
) -> dict[int, list[TextEntry]]:
    """Return every current-locale entry for the requested stable text IDs."""

    wanted = set(text_ids)
    matches = {text_id: [] for text_id in text_ids}
    for entry in entries:
        if entry.text_id in wanted:
            matches[entry.text_id].append(entry)
    return matches


def build_report(
    *,
    locale: str,
    terms: list[str],
    text_ids: list[int],
    exact: bool,
    discovery_window_mb: int,
    fallback_scan_mb: int,
) -> dict[str, object]:
    locale = normalize_locale(locale)
    with ProcessReader() as reader:
        validate_version(reader)
        region, pool_center, anchors, discovery_mode = locate_localization_pool(
            reader,
            _candidate_regions(reader),
            discovery_window_mb=discovery_window_mb,
            fallback_scan_mb=fallback_scan_mb,
        )
        start = max(region.base, pool_center - CURRENT_LANGUAGE_POOL_RADIUS)
        end = min(region.base + region.size, pool_center + CURRENT_LANGUAGE_POOL_RADIUS)
        block = reader.read(start, end - start)
        entries = list(iter_text_entries_from_block(block, start))
        matches = match_entries(entries, terms, exact=exact)
        id_matches = match_text_ids(entries, text_ids)

        return {
            "schema": "nioh3-scroll-auxiliary-text-probe-v1",
            "captured_at_utc": datetime.now(timezone.utc).isoformat(),
            "game_version": "2.00.02",
            "locale": locale,
            "process_id": reader.pid,
            "module_base": f"0x{reader.module_base:016X}",
            "discovery_mode": discovery_mode,
            "pool_center": f"0x{pool_center:016X}",
            "pool_radius": CURRENT_LANGUAGE_POOL_RADIUS,
            "scan_range": [f"0x{start:016X}", f"0x{end:016X}"],
            "anchor_count": len(anchors),
            "plausible_entry_count": len(entries),
            "match_mode": "exact" if exact else "substring",
            "terms": terms,
            "text_ids": [f"0x{text_id:08X}" for text_id in text_ids],
            "matches": {
                term: [
                    {
                        **asdict(entry),
                        "text_id_hex": f"0x{entry.text_id:08X}",
                        "address_hex": f"0x{entry.address:016X}",
                    }
                    for entry in selected
                ]
                for term, selected in matches.items()
            },
            "text_id_matches": {
                f"0x{text_id:08X}": [
                    {
                        **asdict(entry),
                        "text_id_hex": f"0x{entry.text_id:08X}",
                        "address_hex": f"0x{entry.address:016X}",
                    }
                    for entry in selected
                ]
                for text_id, selected in id_matches.items()
            },
        }


def parse_int(value: str) -> int:
    parsed = int(value, 0)
    if not 0 <= parsed <= 0xFFFFFFFF:
        raise argparse.ArgumentTypeError("text ID must fit in uint32")
    return parsed


def parse_args() -> argparse.Namespace:
    parser = argparse.ArgumentParser(
        description=(
            "Read the validated current-language localization pool and find "
            "native scroll special-rule or terrain strings."
        )
    )
    parser.add_argument("--locale", required=True)
    parser.add_argument("--term", action="append", default=[], dest="terms")
    parser.add_argument(
        "--text-id",
        action="append",
        default=[],
        type=parse_int,
        dest="text_ids",
        help="Stable native localization text ID, in decimal or 0x-prefixed form",
    )
    parser.add_argument("--exact", action="store_true")
    parser.add_argument("--discovery-window-mb", type=int, default=8)
    parser.add_argument("--fallback-scan-mb", type=int, default=256)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    if not args.terms and not args.text_ids:
        parser.error("at least one --term or --text-id is required")
    return args


def main() -> int:
    if hasattr(sys.stdout, "reconfigure"):
        sys.stdout.reconfigure(encoding="utf-8", errors="backslashreplace")
    args = parse_args()
    report = build_report(
        locale=args.locale,
        terms=args.terms,
        text_ids=args.text_ids,
        exact=args.exact,
        discovery_window_mb=args.discovery_window_mb,
        fallback_scan_mb=args.fallback_scan_mb,
    )
    args.output.parent.mkdir(parents=True, exist_ok=True)
    temporary = args.output.with_suffix(args.output.suffix + ".tmp")
    temporary.write_text(
        json.dumps(report, ensure_ascii=False, indent=2) + "\n",
        encoding="utf-8",
    )
    temporary.replace(args.output)
    print(f"Wrote {args.output}")
    for term, matches in report["matches"].items():
        print(f"{term}: {len(matches)} match(es)")
        for item in matches:
            print(f"  {item['text_id_hex']}: {item['text']}")
    for text_id, matches in report["text_id_matches"].items():
        print(f"{text_id}: {len(matches)} match(es)")
        for item in matches:
            print(f"  {item['text']}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
