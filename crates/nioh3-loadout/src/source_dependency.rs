//! A bounded guard for legacy flattened source inputs, not a source producer.
//!
//! The legacy path retains a core aggregate and fixed before/after submissions.
//! It cannot follow candidate-dependent inheritance, guardian gates, or late
//! source production. Equality below preserves those known dependencies. The
//! source model remains `legacy_unverified`: unknown prefix ownership, raw tail
//! records and the numeric baseline are not proved by an equal signature.

use super::source_rebuild::SourceOutfit;
use super::*;

pub(crate) const CHANGED: &str =
    "legacy shared-source dependencies changed; fresh source rebuild required";
pub(crate) const ATTRIBUTES: &str =
    "legacy source attributes changed; fresh source rebuild required";

const SENSITIVE: [u16; 7] = [0xD7BF, 0xCB4B, 0x8508, 0x38BA, 0x1EE2, 0xC437, 0xD12B];
const LATE: [u16; 3] = [0x1EE2, 0xC437, 0xD12B];

#[derive(Clone, Debug, PartialEq, Eq)]
struct Shape {
    // Unknown fixed tail submissions share native map capacity256. Preserve
    // the complete ordered key set, not only the currently known gates.
    keys: [Vec<u16>; 2],
    // Membership (including value0), base and dup all matter. Inheritance uses
    // the signed wrapping base+dup; guardian8508 uses key presence alone.
    gates: [Vec<(u16, u32, u16)>; 2],
    // Late raw core sides are unbound in legacy contexts. Preserve full side
    // records, order and capacity20: fixed dedup, levels, reduction, thresholds
    // and later insertion can all depend on this state.
    sides: [Vec<[u8; 68]>; 2],
}

impl Shape {
    fn of(source: &Source) -> Self {
        Self {
            keys: source
                .maps
                .each_ref()
                .map(|map| map.iter().map(|e| e.key).collect()),
            gates: source.maps.each_ref().map(|map| {
                map.iter()
                    .filter(|e| SENSITIVE.contains(&e.key))
                    .map(|e| (e.key, e.base, e.dup))
                    .collect()
            }),
            sides: source.sides.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ItemDependency {
    category: Option<u32>,
    source: Shape,
    // Preserve the raw ordered gate operands too. Equal capped aggregates do
    // not prove equal behavior when merged after a different prefix.
    gates: Vec<(u16, u32)>,
    // Original1075ED0 ignores qualification and ordinary masks. Do not infer
    // these inputs from the collected aggregate or omit zero-valued lines.
    late: Vec<(u16, u32)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Signature {
    attrs: [i32; 7],
    before_guardian: Shape,
    pub(crate) items: Vec<ItemDependency>,
}

fn empty_source(c: &Character, attrs: &[i32; 7]) -> Source {
    Source {
        mode: c.mode,
        thresholds: attrs.map(|value| value as u32),
        maps: [Vec::new(), Vec::new()],
        sides: [Vec::new(), Vec::new()],
        set_reduction: [false; 2],
        set_pieces: [Vec::new(), Vec::new()],
    }
}

impl Tables {
    pub(super) fn collect_source_slot(
        &self,
        source: &mut Source,
        outfit: &SourceOutfit<'_>,
        attrs: &[i32; 7],
        bank: usize,
        slot: usize,
        item: &[u8],
    ) -> R<()> {
        let key = u16_at(item, 0);
        let original = (key != 0).then(|| self.items.row(u32::from(key))).flatten();
        if let Some(row) = original {
            let reserve = bank != outfit.active;
            match slot {
                0..=3 | 13 if self.requirements(outfit.cap, attrs, row, item) => {
                    let held = usize::from(outfit.inventory[bank * BANK + 0xF68] == 0);
                    let mask = if reserve || slot == held || slot == 13 {
                        255
                    } else {
                        0
                    };
                    self.collect(source, item, reserve, bank as i32, mask)?;
                }
                4..=12 if self.requirements(outfit.cap, attrs, row, item) => {
                    self.collect(
                        source,
                        item,
                        reserve,
                        bank as i32,
                        if reserve { 0 } else { 255 },
                    )?;
                }
                15 if !reserve => self.collect(source, item, false, outfit.mode, 255)?,
                15 => {
                    for (_, group, _, value) in self.effect_lines(item) {
                        if matches!(group, 0xF79F | 0x62CB | 0x17B4) {
                            self.aggregate(source, outfit.mode, group, value);
                        }
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Structural contribution before any other row is merged. The planner
    /// compares this for every possible placement, including both accessory
    /// orientations, so jointly activated set tiers cannot hide behind an
    /// unchanged one-item aggregate or local dominance.
    pub(crate) fn legacy_item_dependency(
        &self,
        c: &Character,
        attrs: &[i32; 7],
        bank: usize,
        slot: usize,
        item: &[u8],
    ) -> R<ItemDependency> {
        if item.len() != ITEM || bank > 1 || slot > 15 {
            return Err("legacy source dependency record has invalid extent");
        }
        let mut source = empty_source(c, attrs);
        let key = u16_at(item, 0);
        self.collect_source_slot(&mut source, &SourceOutfit::from(c), attrs, bank, slot, item)?;
        let lines = self.effect_lines(item);
        Ok(ItemDependency {
            category: if matches!(slot, 0..=3 | 13) {
                self.category(key)
            } else {
                None
            },
            source: Shape::of(&source),
            gates: lines
                .iter()
                .filter(|(_, group, _, _)| SENSITIVE.contains(group))
                .map(|&(_, group, _, value)| (group, value))
                .collect(),
            late: if matches!(slot, 13 | 9 | 10 | 15) {
                lines
                    .iter()
                    .filter(|(_, group, _, _)| LATE.contains(group))
                    .map(|&(_, group, _, value)| (group, value))
                    .collect()
            } else {
                Vec::new()
            },
        })
    }

    pub(crate) fn legacy_source_signature(
        &self,
        c: &Character,
        attrs: &[i32; 7],
        source: &Source,
    ) -> R<Signature> {
        if c.active > 1 || c.inv.len() < 2 * BANK {
            return Err("legacy source dependency inventory has invalid extent");
        }
        let mut items = Vec::with_capacity(32);
        for bank in 0..2 {
            for slot in 0..16 {
                let at = bank * BANK + slot * ITEM;
                items.push(self.legacy_item_dependency(
                    c,
                    attrs,
                    bank,
                    slot,
                    &c.inv[at..at + ITEM],
                )?);
            }
        }
        Ok(Signature {
            attrs: *attrs,
            before_guardian: Shape::of(source),
            items,
        })
    }
}
