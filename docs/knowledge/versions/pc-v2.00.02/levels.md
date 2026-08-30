# Scroll level and recommended level

This document deliberately separates two fields that the UI presents together.

## Scroll item level (`record +0x06`, mirror `+0x08`)

Evidence grade: `native-control-flow` for effect values and exchange packing;
`unknown` for challenge enemy scaling.

- This is the `Lv.` shown on the scroll and the `level` input used by the
  recovered effect-value formula at RVA `0x571478`.
- It can change numeric effect values through the native level curve and
  normalization. It does not choose the effect ID by itself.
- The exchange serializer uses `min(record_level, 180)` as the outbound
  effective level, unless flag `0x00200000` at record `+0x18` forces zero.
- No recovered challenge-construction path currently proves that `+0x06/+0x08`
  controls enemy or Boss combat level.

## Recommended internal level (`record +0x10`, mirror `+0x12`)

Evidence grade: `native-table` + `native-control-flow` for conversion;
`observed/inferred` for exact combatant-level equality.

The constructor clamps the internal value to 156..1400. The detail UI and
challenge descriptor call RVA `0x5702E0` and evaluate a captured 42-point curve
using float32 linear interpolation and truncation toward zero. Values at and
above the final curve region display as 700; for example, a requested 1500 is
first clamped to 1400 and displays 700.

The canonical machine resource is
`nioh3_scroll_editor/data/recommended_level_curve.json`; the implementation is
`nioh3_scroll_editor/recommended_level.py`.

Static challenge-descriptor evidence:

- RVA `0x20DD600` reads record `+0x12` and passes it to `0x20DD588`;
- `0x20DD588` writes the internal value to descriptor `+0x28` and the converted
  display value to descriptor `+0x2C`;
- this chain constructs terrain/enemy/rule descriptors and does not read
  record `+0x06/+0x08`.

What remains unproven is the final read from descriptor `+0x2C` into enemy/Boss
AI level initialization. In-game observations strongly correlate them, so the
product may present the converted value as an expected challenge level, but it
must not call it a byte-proved exact Boss-level control until that consumer is
closed.

## Practical interpretation

- Lower recommended values produce lower displayed challenge recommendations;
  higher values approach the displayed cap of 700.
- Scroll item level should be chosen for desired effect-value scaling and
  canonical outbound behavior, not as a documented enemy-level slider.
- The application should show both the requested internal value and its final
  displayed recommendation so the user sees the clamp before writing.

## Evidence

- dynamic curve capture: `audit/p1_dynamic/recommended_level_curve_20260830/`
- static callers: `audit/p1_static/recommended_level_curve_callers_20260830.json`
- detail consumer: `audit/p1_static/scroll_detail_consumer_1F0B3D4_20260829.json`
- challenge constructor: `audit/p1_static/aux_ctor_20DD430_20DD760.asm`
- regression tests: `test_recommended_level.py`
