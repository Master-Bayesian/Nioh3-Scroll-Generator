//! `recommended_level.resolve` and its metadata.
//!
//! Ports `nioh3_scroll_editor/recommended_level.py` exactly: the captured
//! native curve, the float32-linear-truncate forward prediction, and the exact
//! inverse that never rounds or clamps a requested displayed level. The
//! resource is the shipped `data/recommended_level_curve.json`.

use std::fs;
use std::path::Path;

use crate::engine::EngineError;

/// `recommended_level.native_recommended_level_curve` schema string.
const CURVE_SCHEMA: &str = "nioh3-recommended-level-curve/v1";

/// The captured native curve plus its precomputed canonical display table.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendedLevelCurve {
    minimum_internal_level: i32,
    maximum_internal_level: i32,
    points: Vec<(i32, i32)>,
    canonical_displayed: Vec<i32>,
}

/// `RecommendedLevelResolution`, projected onto the wire shape (plus metadata).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecommendedLevelResolution {
    pub requested_displayed_level: i32,
    pub status: &'static str,
    pub canonical_internal_levels: Vec<i32>,
    pub selected_internal_level: Option<i32>,
}

impl RecommendedLevelCurve {
    /// `RecommendedLevelCurve.canonical_internal_level`: plain clamping.
    pub fn canonical_internal_level(&self, requested: i32) -> i32 {
        requested.clamp(self.minimum_internal_level, self.maximum_internal_level)
    }

    /// `RecommendedLevelCurve.displayed_level`: bisect then one float32-linear
    /// step, truncated toward zero, with the reference's out-of-range clamps.
    pub fn displayed_level(&self, internal_level: i32) -> i32 {
        let upper = self
            .points
            .partition_point(|point| point.0 <= internal_level);
        if upper == 0 {
            return self.points[0].1;
        }
        if upper >= self.points.len() {
            return self.points[self.points.len() - 1].1;
        }
        let (lower_input, lower_display) = self.points[upper - 1];
        let (upper_input, upper_display) = self.points[upper];
        if upper_input <= lower_input {
            return lower_display;
        }
        let factor = ((internal_level - lower_input) as f32) / ((upper_input - lower_input) as f32);
        let delta = ((upper_display - lower_display) as f32) * factor;
        lower_display + delta.trunc() as i32
    }

    /// `displayed_level_bounds` over the canonical internal range.
    pub fn displayed_level_bounds(&self) -> (i32, i32) {
        let minimum = *self
            .canonical_displayed
            .iter()
            .min()
            .expect("the canonical range is never empty");
        let maximum = *self
            .canonical_displayed
            .iter()
            .max()
            .expect("the canonical range is never empty");
        (minimum, maximum)
    }

    /// `RecommendedLevelCurve.resolve_displayed_level`.
    pub fn resolve_displayed_level(&self, requested: i32) -> RecommendedLevelResolution {
        let matches: Vec<i32> = self
            .canonical_displayed
            .iter()
            .enumerate()
            .filter(|(_, displayed)| **displayed == requested)
            .map(|(index, _)| self.minimum_internal_level + index as i32)
            .collect();
        let (minimum, maximum) = self.displayed_level_bounds();
        let status = if !matches.is_empty() {
            "exact"
        } else if requested < minimum || requested > maximum {
            "out_of_range"
        } else {
            "unreachable"
        };
        RecommendedLevelResolution {
            requested_displayed_level: requested,
            status,
            selected_internal_level: matches.first().copied(),
            canonical_internal_levels: matches,
        }
    }

    /// `catalog_application.recommended_level_metadata`.
    pub fn metadata(&self) -> serde_json::Value {
        let (minimum, maximum) = self.displayed_level_bounds();
        serde_json::json!({
            "minimum_internal_level": self.minimum_internal_level,
            "maximum_internal_level": self.maximum_internal_level,
            "minimum_displayed_level": minimum,
            "maximum_displayed_level": maximum,
            "selection_policy": "lowest_canonical_internal_level",
            "evidence": "captured_native_curve_prediction",
        })
    }

    /// `catalog_application.resolve_recommended_level_payload`.
    pub fn resolve_payload(&self, displayed_level: i32) -> serde_json::Value {
        let resolution = self.resolve_displayed_level(displayed_level);
        serde_json::json!({
            "requested_displayed_level": resolution.requested_displayed_level,
            "status": resolution.status,
            "canonical_internal_levels": resolution.canonical_internal_levels,
            "selected_internal_level": resolution.selected_internal_level,
            "metadata": self.metadata(),
        })
    }
}

/// Load and validate `data/recommended_level_curve.json`.
pub fn load(data_root: &Path) -> Result<RecommendedLevelCurve, EngineError> {
    let path = data_root.join("recommended_level_curve.json");
    let text = fs::read_to_string(&path)
        .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
    let value: serde_json::Value = serde_json::from_str(&text)
        .map_err(|error| EngineError::new("RESOURCE_MISMATCH", error.to_string()))?;
    if value.get("schema").and_then(serde_json::Value::as_str) != Some(CURVE_SCHEMA) {
        return Err(EngineError::new(
            "INVALID_REQUEST",
            "unsupported recommended-level curve schema",
        ));
    }
    let constructor = value.get("record_constructor").ok_or_else(|| {
        EngineError::new(
            "INVALID_REQUEST",
            "recommended-level curve points are invalid",
        )
    })?;
    let minimum_internal_level = constructor
        .get("minimum_internal_level")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            EngineError::new(
                "INVALID_REQUEST",
                "recommended-level curve points are invalid",
            )
        })? as i32;
    let maximum_internal_level = constructor
        .get("maximum_internal_level")
        .and_then(serde_json::Value::as_i64)
        .ok_or_else(|| {
            EngineError::new(
                "INVALID_REQUEST",
                "recommended-level curve points are invalid",
            )
        })? as i32;

    let mut points: Vec<(i32, i32)> = Vec::new();
    let raw_points = value
        .get("curve")
        .and_then(|curve| curve.get("points"))
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| {
            EngineError::new(
                "INVALID_REQUEST",
                "recommended-level curve points are invalid",
            )
        })?;
    for raw in raw_points {
        let pair = raw
            .as_array()
            .filter(|pair| pair.len() == 2)
            .ok_or_else(|| {
                EngineError::new(
                    "INVALID_REQUEST",
                    "recommended-level curve points are invalid",
                )
            })?;
        let input = pair[0].as_i64().ok_or_else(|| {
            EngineError::new(
                "INVALID_REQUEST",
                "recommended-level curve points are invalid",
            )
        })? as i32;
        let display = pair[1].as_i64().ok_or_else(|| {
            EngineError::new(
                "INVALID_REQUEST",
                "recommended-level curve points are invalid",
            )
        })? as i32;
        points.push((input, display));
    }
    if points.len() < 2 || points.windows(2).any(|pair| pair[1].0 < pair[0].0) {
        return Err(EngineError::new(
            "INVALID_REQUEST",
            "recommended-level curve points are invalid",
        ));
    }
    if maximum_internal_level < minimum_internal_level {
        return Err(EngineError::new(
            "INVALID_REQUEST",
            "recommended-level curve points are invalid",
        ));
    }

    let mut curve = RecommendedLevelCurve {
        minimum_internal_level,
        maximum_internal_level,
        points,
        canonical_displayed: Vec::new(),
    };
    curve.canonical_displayed = (minimum_internal_level..=maximum_internal_level)
        .map(|internal| curve.displayed_level(internal))
        .collect();
    Ok(curve)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn curve() -> RecommendedLevelCurve {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../nioh3_scroll_editor/data");
        load(&root).expect("load the shipped recommended-level curve")
    }

    /// Values captured from the shipped Python curve on this machine.
    #[test]
    fn forward_curve_matches_the_captured_python_values() {
        let curve = curve();
        assert_eq!(curve.minimum_internal_level, 156);
        // The current supported bound is raw 600 -> displayed 356.
        assert_eq!(curve.maximum_internal_level, 600);
        assert_eq!(curve.displayed_level_bounds(), (142, 356));
        for (raw, expected) in [
            (0, 142),
            (155, 142),
            (156, 142),
            (157, 142),
            (160, 145),
            (170, 154),
            (180, 159),
            (183, 160),
            (200, 169),
            (230, 180),
            (300, 214),
            (500, 292),
            (599, 356),
            (600, 356),
            // Above the bound the constructor clamp is what moves the value.
            (601, 356),
            (1400, 356),
        ] {
            assert_eq!(
                curve.displayed_level(curve.canonical_internal_level(raw)),
                expected,
                "internal {raw}"
            );
        }
    }

    /// The captured curve points survive the bound change untouched.
    #[test]
    fn the_curve_points_still_carry_the_legacy_display_mapping() {
        let curve = curve();
        assert_eq!(curve.displayed_level(1000), 549);
        assert_eq!(curve.displayed_level(1300), 699);
        assert_eq!(curve.displayed_level(1301), 700);
        assert_eq!(curve.displayed_level(1400), 700);
    }

    #[test]
    fn inverse_resolution_matches_the_captured_python_values() {
        let curve = curve();
        let cases: [(i32, &str, Vec<i32>, Option<i32>); 8] = [
            (-5, "out_of_range", Vec::new(), None),
            (0, "out_of_range", Vec::new(), None),
            (137, "out_of_range", Vec::new(), None),
            (138, "out_of_range", Vec::new(), None),
            (200, "exact", vec![272, 273], Some(272)),
            (250, "exact", vec![420], Some(420)),
            (313, "exact", vec![521], Some(521)),
            // Display 356 is the top of the canonical range, reached by 599/600.
            (356, "exact", vec![599, 600], Some(599)),
        ];
        for (requested, status, alternatives, selected) in cases {
            let resolution = curve.resolve_displayed_level(requested);
            assert_eq!(resolution.status, status, "requested {requested}");
            assert_eq!(
                resolution.canonical_internal_levels, alternatives,
                "requested {requested}"
            );
            assert_eq!(
                resolution.selected_internal_level, selected,
                "requested {requested}"
            );
        }
        // 530 sat exactly under the old bound and is out of range now; the
        // legacy plateau is still inside the captured points but is no longer
        // reachable through the bound-limited inverse.
        for requested in [530, 357, 700, 701] {
            let resolution = curve.resolve_displayed_level(requested);
            assert_eq!(resolution.status, "out_of_range", "requested {requested}");
            assert!(resolution.canonical_internal_levels.is_empty());
            assert_eq!(resolution.selected_internal_level, None);
        }
    }

    #[test]
    fn a_gap_inside_the_bounds_is_unreachable_not_clamped() {
        let curve = curve();
        let (minimum, maximum) = curve.displayed_level_bounds();
        let reached: std::collections::BTreeSet<i32> =
            curve.canonical_displayed.iter().copied().collect();
        let gaps: Vec<i32> = (minimum..=maximum)
            .filter(|value| !reached.contains(value))
            .collect();
        // The shipped curve has exactly one unreachable displayed value.
        assert_eq!(gaps, vec![328]);
        let resolution = curve.resolve_displayed_level(328);
        assert_eq!(resolution.status, "unreachable");
        assert!(resolution.canonical_internal_levels.is_empty());
        assert_eq!(resolution.selected_internal_level, None);
    }

    #[test]
    fn metadata_matches_the_shipped_payload() {
        let curve = curve();
        assert_eq!(
            curve.metadata(),
            serde_json::json!({
                "minimum_internal_level": 156,
                "maximum_internal_level": 600,
                "minimum_displayed_level": 142,
                "maximum_displayed_level": 356,
                "selection_policy": "lowest_canonical_internal_level",
                "evidence": "captured_native_curve_prediction",
            })
        );
    }
}
