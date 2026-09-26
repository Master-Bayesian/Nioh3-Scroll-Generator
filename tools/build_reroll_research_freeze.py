from __future__ import annotations

"""Build a sanitized, reproducible reroll-research freeze archive."""

from datetime import date
import hashlib
from pathlib import Path
import zipfile


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "deliverables" / "Nioh3_Reroll_Research_Freeze_20260831_REVISED.zip"

FILES = (
    "docs/knowledge/versions/pc-v2.00.02/README.md",
    "docs/knowledge/versions/pc-v2.00.02/evidence-register.md",
    "docs/knowledge/versions/pc-v2.00.02/reroll-generation.md",
    "nioh3_scroll_editor/reroll.py",
    "nioh3_scroll_editor/effect_generation_tables.py",
    "nioh3_scroll_editor/r4_finalizer_reference.py",
    "nioh3_scroll_editor/r4_finalizer_resource.py",
    "nioh3_scroll_editor/r4_table_bundle.py",
    "research/predict_scroll_rerolls.py",
    "research/analyze_scroll_reroll_capture.py",
    "research/capture_scroll_reroll_ce.lua",
    "research/capture_scroll_save_record.py",
    "research/find_live_scroll_records.py",
    "test_reroll.py",
    "test_challenge_completion_capture.py",
    "audit/p1_static/reroll_candidate_pipeline_20260831.json",
    "audit/p1_static/reroll_effect_caller_xrefs_20260831.json",
    "audit/p1_static/reroll_effect_helper_callers_20260831.json",
    "audit/p1_static/reroll_effect_helper_xrefs_20260831.json",
    "audit/p1_static/reroll_remaining_helpers_20260831.json",
    "audit/p1_static/reroll_rng_helpers_20260831.json",
    "audit/p1_static/reroll_state_functions_20260831.json",
    "captures/reroll_live/seed_203900415/controlled_completion_vector.json",
    "captures/reroll_live/seed_203900415/screenshots/01_pre_challenge_record.png",
    "captures/reroll_live/seed_203900415/screenshots/02_slot2_body_mystic.png",
    "captures/reroll_live/seed_203900415/screenshots/03_slot3_martial_mystic.png",
    "captures/reroll_live/seed_203900415/screenshots/04_slot4_onmyo_item.png",
    "captures/reroll_live/seed_203900415/screenshots/05_slot5_essence_gauge.png",
)

RESOURCE_ROOT = Path(
    "nioh3_scroll_editor/data/r4_finalizer/pc_v2_00_02/resource_v1"
)


def sha256(data: bytes) -> str:
    return hashlib.sha256(data).hexdigest().upper()


def write_entry(archive: zipfile.ZipFile, name: str, data: bytes) -> None:
    info = zipfile.ZipInfo(name, date_time=(2026, 8, 31, 0, 0, 0))
    info.compress_type = zipfile.ZIP_DEFLATED
    info.create_system = 3
    info.external_attr = 0o100644 << 16
    archive.writestr(info, data)


def main() -> None:
    entries = [Path(item) for item in FILES]
    entries.extend(
        path.relative_to(ROOT)
        for path in sorted((ROOT / RESOURCE_ROOT).rglob("*"))
        if path.is_file()
    )
    missing = [str(path) for path in entries if not (ROOT / path).is_file()]
    if missing:
        raise FileNotFoundError("missing freeze inputs: " + ", ".join(missing))

    payloads = {path.as_posix(): (ROOT / path).read_bytes() for path in entries}
    checksums = "".join(
        f"{sha256(data)}  {name}\n" for name, data in sorted(payloads.items())
    ).encode("utf-8")
    manifest = f"""# Nioh 3 reroll research freeze — 2026-08-31

## Scope

This archive freezes the PC v2.00.02 reroll work at its current evidence
boundary.  It contains no encrypted save and no Steam account path.

## Confirmed

- Paid/manual candidate builder RVA `0x20C4BD0` is reconstructed as an offline
  static candidate model, including Seed plus counter RNG scoping, weighted
  pool construction, conflicts, capacity checks, and up to five unique groups.
- Seed `203900415` is a clean controlled post-challenge vector with four
  per-secondary-slot candidates.  Accepting slot 2 changed `0xD411` to
  canonical `0xDAC2`, advanced counter 1 to 2, and was verified in a read-only
  decrypted-save comparison.
- The controlled completion vector and all 12 changed record bytes are stored
  in `captures/reroll_live/seed_203900415/controlled_completion_vector.json`.
- Full repository test discovery passed 352 tests on {date.today().isoformat()}.

## Frozen unresolved boundary

- The post-challenge four-slot vector does not match the paid/manual
  five-candidate predictor under any enumerated dynamic-gate subset.
- The challenge-completion generator entry, its extra state, and its exact RNG
  sequence are not yet recovered.
- The slot-5 UI proves the Anima Charge Bonus group and `+9.3%` display value,
  but not a unique effect ID; the earlier `0xBC51` guess is withdrawn.
- No player-facing guaranteed reroll prediction is authorized by this freeze.
- Seed `99183032` is excluded because the input record was previously modified.

## Resume point

Recover the post-challenge generator separately, then require another clean
vector and exact native-order parity before integrating prediction into the
application.
""".encode("utf-8")

    OUTPUT.parent.mkdir(parents=True, exist_ok=True)
    if OUTPUT.exists():
        raise FileExistsError(f"refusing to overwrite existing freeze: {OUTPUT}")
    with zipfile.ZipFile(OUTPUT, "w", compression=zipfile.ZIP_DEFLATED) as archive:
        write_entry(archive, "FREEZE_MANIFEST.md", manifest)
        write_entry(archive, "SHA256SUMS.txt", checksums)
        for name, data in sorted(payloads.items()):
            write_entry(archive, name, data)

    digest = sha256(OUTPUT.read_bytes())
    sidecar = OUTPUT.with_suffix(OUTPUT.suffix + ".sha256")
    sidecar.write_text(f"{digest}  {OUTPUT.name}\n", encoding="ascii")
    print(OUTPUT)
    print(f"files={len(payloads) + 2}")
    print(f"sha256={digest}")


if __name__ == "__main__":
    main()
