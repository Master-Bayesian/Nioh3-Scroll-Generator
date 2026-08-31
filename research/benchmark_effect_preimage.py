"""Run the complete-effect inversion parity benchmark on one D3D11 adapter."""

from __future__ import annotations

import argparse
import json
from pathlib import Path
import sys
from time import perf_counter

PROJECT_ROOT = Path(__file__).resolve().parents[1]
if str(PROJECT_ROOT) not in sys.path:
    sys.path.insert(0, str(PROJECT_ROOT))

from nioh3_scroll_editor.effect_path_inverse import (
    FullCompositionRequest,
    compile_full_composition_plans,
    verify_complete_matches,
)
from nioh3_scroll_editor.effect_preimage_accelerator import (
    AMD_VENDOR_ID,
    INTEL_VENDOR_ID,
    NVIDIA_VENDOR_ID,
    collect_effect_preimage_matches_d3d11,
    d3d11_effect_adapter_info,
)


EXPECTED_SEEDS = (
    1,
    7_898_609,
    25_934_837,
    29_849_823,
    33_957_113,
    48_135_696,
    76_175_780,
    94_647_132,
    250_107_693,
)
VENDORS = {
    "auto": 0,
    "amd": AMD_VENDOR_ID,
    "nvidia": NVIDIA_VENDOR_ID,
    "intel": INTEL_VENDOR_ID,
}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--vendor", choices=tuple(VENDORS), default="auto")
    parser.add_argument("--output", type=Path)
    arguments = parser.parse_args()

    vendor_id = VENDORS[arguments.vendor]
    adapter = d3d11_effect_adapter_info(vendor_id=vendor_id)
    if adapter is None:
        raise SystemExit(f"No Direct3D 11 adapter matched vendor {arguments.vendor!r}")
    request = FullCompositionRequest(
        5,
        0xA051,
        (0xD40A, 0x34F3, 0x3E7A, 0xAE5A),
        0x6553,
    )
    plan = compile_full_composition_plans(request)[0]
    started = perf_counter()
    matches = collect_effect_preimage_matches_d3d11(
        plan,
        vendor_id=vendor_id,
        output_capacity=1_000,
    )
    elapsed = perf_counter() - started
    if matches is None:
        raise SystemExit("The Direct3D 11 accelerator DLL is unavailable")
    scanner_seeds = tuple(sorted(seed for seed, _trial in matches))
    verified_seeds = verify_complete_matches(request, scanner_seeds)
    passed = scanner_seeds == EXPECTED_SEEDS and verified_seeds == EXPECTED_SEEDS
    payload = {
        "schema": "nioh3.effect-preimage-benchmark.v1",
        "adapter": {
            "description": adapter.description,
            "vendor_id": f"0x{adapter.vendor_id:04X}",
            "device_id": f"0x{adapter.device_id:04X}",
            "dedicated_video_memory": adapter.dedicated_video_memory,
            "shared_system_memory": adapter.shared_system_memory,
        },
        "case": "r5_seed1_full_composition",
        "pivot_draw_index": plan.pivot_draw_index,
        "pivot_state_count": plan.pivot_state_count,
        "compiled_path_count": len(plan.paths),
        "scanner_seed_count": len(scanner_seeds),
        "verified_seed_count": len(verified_seeds),
        "seconds": elapsed,
        "rate_million_states_per_second": plan.pivot_state_count / elapsed / 1e6,
        "scanner_seeds": scanner_seeds,
        "verified_seeds": verified_seeds,
        "expected_seeds": EXPECTED_SEEDS,
        "passed": passed,
    }
    rendered = json.dumps(payload, ensure_ascii=False, indent=2) + "\n"
    print(rendered, end="")
    if arguments.output is not None:
        arguments.output.parent.mkdir(parents=True, exist_ok=True)
        arguments.output.write_text(rendered, encoding="utf-8")
    return 0 if passed else 1


if __name__ == "__main__":
    raise SystemExit(main())
