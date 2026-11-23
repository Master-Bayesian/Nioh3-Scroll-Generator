//! Tiny exhaustive reference for fixed-attribute CPU searches. The input
//! inventory is resolved independently of the planner and the production wear path.
use super::{Backpack, Candidate, Engine, Outcome, BANK, ITEM, SLOTS};
use crate::pipeline::{SetRowActivation, SetState};
use crate::search::Request;
use serde::Serialize;
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

const BANKS: usize = 2;
const PHYSICAL_SLOTS: usize = 16;
const ACCESSORIES: [usize; 2] = [9, 10];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct OwnedId(u32);

type Owned = BTreeMap<OwnedId, [u8; ITEM]>;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize)]
enum SlotChoice {
    Keep,
    Empty,
    Equip(OwnedId),
}

use SlotChoice::{Empty, Equip, Keep};

type Assignment = [SlotChoice; PHYSICAL_SLOTS];
type EquippedWords = [[Vec<u32>; PHYSICAL_SLOTS]; BANKS];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct PhysicalLayout([[Option<OwnedId>; PHYSICAL_SLOTS]; BANKS]);

impl PhysicalLayout {
    fn has_distinct_identities(&self) -> bool {
        let mut seen = BTreeSet::new();
        self.0.iter().flatten().flatten().all(|id| seen.insert(id))
    }

    fn resolve(self, bank: usize, choices: &Assignment) -> Option<Self> {
        assert!(
            self.has_distinct_identities(),
            "duplicate identity in before layout"
        );
        let before = self.0[bank];
        let mut after = self;
        after.0[bank] = std::array::from_fn(|slot| match choices[slot] {
            Keep => before[slot],
            Empty => None,
            Equip(id) => Some(id),
        });
        if let Equip(id) = choices[0] {
            if let Some(spare) = (1..=3).find(|&slot| before[slot] == Some(id)) {
                // A swap assigns the displaced slot as part of the same choice.
                if choices[spare] != Keep {
                    return None;
                }
                after.0[bank][0] = before[spare];
                after.0[bank][spare] = before[0];
            }
        }
        after.has_distinct_identities().then_some(after)
    }

    fn inventory(&self, base: &[u8], owned: &Owned) -> Vec<u8> {
        let mut inventory = base.to_vec();
        for (bank, slots) in self.0.iter().enumerate() {
            for (slot, id) in slots.iter().enumerate() {
                let at = bank * BANK + slot * ITEM;
                let record = &mut inventory[at..at + ITEM];
                if let Some(id) = id {
                    record.copy_from_slice(&owned[id]);
                } else if record[..2] != [0, 0] {
                    record.fill(0);
                }
                // A captured empty slot can carry nonzero padding. Preserve
                // those words when the slot was already empty.
            }
        }
        inventory
    }
}

fn equipped_words(inventory: &[u8]) -> EquippedWords {
    std::array::from_fn(|bank| {
        std::array::from_fn(|slot| {
            let at = bank * BANK + slot * ITEM;
            inventory[at..at + ITEM]
                .as_chunks::<4>()
                .0
                .iter()
                .copied()
                .map(u32::from_le_bytes)
                .collect()
        })
    })
}

struct Domain {
    axes: [Vec<SlotChoice>; PHYSICAL_SLOTS],
    accessory_pairs: Option<Vec<[SlotChoice; 2]>>,
}

impl Default for Domain {
    fn default() -> Self {
        Self {
            axes: std::array::from_fn(|_| vec![Keep]),
            accessory_pairs: None,
        }
    }
}

#[derive(Clone, Copy, Default)]
struct AccessoryPolicy {
    allow_empty: bool,
    allow_keep: bool,
}

impl Domain {
    fn accessories(
        mut self,
        eligible: &[OwnedId],
        current: [Option<OwnedId>; 2],
        policy: AccessoryPolicy,
    ) -> Self {
        let options: [Vec<_>; 2] = current.map(|id| {
            let mut options: Vec<_> = eligible
                .iter()
                .copied()
                .map(|eligible| {
                    if policy.allow_keep && id == Some(eligible) {
                        Keep
                    } else {
                        Equip(eligible)
                    }
                })
                .collect();
            if eligible.len() < 2 || policy.allow_empty {
                options.push(Empty);
            }
            if policy.allow_keep && id.is_some_and(|id| !eligible.contains(&id)) {
                options.push(Keep);
            }
            options
        });
        let mut pairs = Vec::new();
        for &first in &options[0] {
            for &second in &options[1] {
                // A one-item mandatory loadout has exactly one vacancy.
                if eligible.len() == 1 && !policy.allow_empty && [first, second] == [Empty, Empty] {
                    continue;
                }
                pairs.push([first, second]);
            }
        }
        self.accessory_pairs = Some(pairs);
        self
    }

    fn assignments(&self) -> Vec<Assignment> {
        let mut rows = vec![[Keep; PHYSICAL_SLOTS]];
        for (slot, options) in self.axes.iter().enumerate() {
            rows = rows
                .iter()
                .flat_map(|prefix| {
                    options.iter().map(move |&choice| {
                        let mut row = *prefix;
                        row[slot] = choice;
                        row
                    })
                })
                .collect();
        }
        if let Some(pairs) = &self.accessory_pairs {
            rows = rows
                .iter()
                .flat_map(|prefix| {
                    pairs.iter().map(move |&[one, two]| {
                        let mut row = *prefix;
                        row[ACCESSORIES[0]] = one;
                        row[ACCESSORIES[1]] = two;
                        row
                    })
                })
                .collect();
        }
        rows
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
enum NativeOutcome {
    Damage(u32),
    Refused(&'static str),
    NoPath,
    OutsideLimits,
}

impl From<Outcome> for NativeOutcome {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Damage(bits) => Self::Damage(bits),
            Outcome::Refused(reason) => Self::Refused(reason),
            Outcome::NoPath => Self::NoPath,
            Outcome::OutsideLimits => Self::OutsideLimits,
        }
    }
}

impl NativeOutcome {
    fn score_bits(self) -> Option<u32> {
        let Self::Damage(bits) = self else {
            return None;
        };
        let score = f32::from_bits(bits);
        if score == 0.0 {
            Some(0)
        } else if score.is_finite() && score > 0.0 {
            Some(bits)
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize)]
struct Evaluated {
    choices: Assignment,
    layout: PhysicalLayout,
    after_words: EquippedWords,
    activations: Vec<SetRowActivation>,
    outcome: NativeOutcome,
    score_bits: Option<u32>,
}

// Each index in rows belongs to exactly one bucket, including invalid damage
// words in refused. Illegal identity assignments are outside the domain.
#[derive(Debug, Default, PartialEq, Eq, Serialize)]
struct Coverage {
    answered: Vec<usize>,
    refused: Vec<usize>,
    no_path: Vec<usize>,
    outside_limits: Vec<usize>,
}

impl Coverage {
    fn record(&mut self, index: usize, outcome: NativeOutcome) {
        match outcome {
            NativeOutcome::Damage(_) if outcome.score_bits().is_some() => &mut self.answered,
            NativeOutcome::Damage(_) | NativeOutcome::Refused(_) => &mut self.refused,
            NativeOutcome::NoPath => &mut self.no_path,
            NativeOutcome::OutsideLimits => &mut self.outside_limits,
        }
        .push(index);
    }
}

#[derive(Debug, Serialize)]
struct Receipt {
    active_bank: usize,
    before: PhysicalLayout,
    before_words: EquippedWords,
    rows: Vec<Evaluated>,
    coverage: Coverage,
    top: Vec<usize>,
}

fn admits(request: &Request, state: &SetState) -> bool {
    request.sets.iter().all(|rule| {
        rule.options
            .iter()
            .filter(|option| {
                state.pieces.iter().any(|&(key, count)| {
                    key == option.key
                        && count != 0
                        && count + usize::from(state.reduction) >= option.pieces
                })
            })
            .count()
            >= rule.at_least
    })
}

fn run(
    engine: &Engine,
    owned: &Owned,
    before: PhysicalLayout,
    domain: &Domain,
    request: &Request,
    top_k: usize,
) -> Receipt {
    assert!(!request.move_points && request.threads == 1);
    assert!(before.has_distinct_identities());
    let character = engine.character();
    let bank = character.active;
    assert_eq!(before.inventory(&character.inv, owned), character.inv);
    let baseline = Candidate {
        attrs: engine.start_attrs,
        items: [None; SLOTS],
    };
    let pack = Backpack::from_slots(&[]).unwrap();
    let mut work = engine.work();
    let mut rows = Vec::new();
    let mut coverage = Coverage::default();
    for choices in domain.assignments() {
        let Some(layout) = before.resolve(bank, &choices) else {
            continue;
        };
        work.resolved_inventory = Some(layout.inventory(&character.inv, owned));
        work.scratch.source.set_row_activations.clear();
        let result = engine.prepare_for_search(
            &baseline,
            &pack,
            &request.limits,
            &mut work,
            (!request.sets.is_empty()).then_some(|state: &SetState| admits(request, state)),
        );
        let outcome = match result {
            Ok(prepared) => prepared.evaluate(),
            Err(outcome) => outcome,
        }
        .into();
        assert_eq!(
            work.resolved_inventory.as_deref(),
            Some(work.character.inv.as_slice())
        );
        coverage.record(rows.len(), outcome);
        rows.push(Evaluated {
            choices,
            layout,
            after_words: equipped_words(&work.character.inv),
            activations: work.scratch.source.set_row_activations.clone(),
            outcome,
            score_bits: outcome.score_bits(),
        });
    }
    let mut top = coverage.answered.clone();
    top.sort_by_key(|&index| {
        let row = &rows[index];
        // Keep the existing 4..12, 0 precedence for searched slots. Extra
        // physical slots follow, and integral owned IDs replace pool offsets.
        let order = [4, 5, 6, 7, 8, 9, 10, 11, 12, 0, 1, 2, 3, 13, 14, 15];
        (
            Reverse(row.score_bits.unwrap()),
            order.map(|slot| row.choices[slot]),
            row.layout,
            index,
        )
    });
    top.truncate(top_k);
    Receipt {
        active_bank: bank,
        before,
        before_words: equipped_words(&character.inv),
        rows,
        coverage,
        top,
    }
}

#[path = "physical_oracle_tests.rs"]
mod tests;
