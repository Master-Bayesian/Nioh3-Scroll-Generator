//! PC202 Samurai high-stance Water Sword, held from full Ki against a grounded target.
use crate::action_rows::{ActionController, CollisionComponent, CollisionRoute};
use crate::arts_proficiency::{ActiveArts, KiPhase};
use serde::Deserialize;

pub const RESOURCE: &str = "0x60000019f";
pub const FIRST: u16 = 25329;
pub const PARAMETERS: [&str; 10] = [
    "9bd7c0721f09cd5466341ad8b752f5718da7ffa2e85ed18bdb7a4a9a0654e73f",
    "a0ea5956ca98c5ed2067897853c968c36890ba8b435389de109f593f144cf7e2",
    "91bd10122bcab771535542e37b3367f79f2eadb141cc648991ae0a7b72f6ef4e",
    "9d2ff52899e3df23e2c377ac76b74f4c3657107c3067dfd74b8047a3a59f0736",
    "51aa21a906b09fa8fb8aa8f7e4d1d76d486e189063c65fe9a8480f367e6ce6c6",
    "9c5b51c2532edb5499123cbd906bce9da858fefedbfe0920f607f14152e93783",
    "652a42e60b7367e059845fd17478f030e5769d6e64cbd7006d30271afdd57b6f",
    "abd0b522216ffc7953341f0a5de0d61b932c8ddaa2a1b132730d71c5335a6022",
    "08a8a8f1e3518dbfb2d389f190359203008c8973707ecb8ef8c8229354740f53",
    "0ad6b5a04375ddb16a0a4538ac13b57e8d6d83653bb4abd6b8818e5fbda9c13e",
];

// Phase pages live only in the projected scorer image. A pointer in the packet
// selects the damage row; the actor controller independently selects the parameter.
pub const PHASE_PAGES: u64 = 0x6000_1000_0000;
pub const CONTROLLER_LINK: u64 = 0x1c5_fdb8_c858;
pub const CURRENT_KI: u64 = 0x1c6_28b4_77fc;
pub const ROLE_FACTS: u64 = PHASE_PAGES + 0x80;
#[derive(Clone, Copy, Debug, Default, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Objective {
    #[default]
    UntilDepleted,
    OpeningThreeActions,
}

impl Objective {
    pub fn validate(self, role: Role) -> Result<(), &'static str> {
        if self == Self::UntilDepleted
            && role
                .effect
                .is_some_and(|arts| arts.ki_phase != KiPhase::Charging)
        {
            return Err("ARTS_KI_DOES_NOT_DEPLETE");
        }
        Ok(())
    }
}
const PAGE: usize = 65536;
const ROWS: usize = 0x4000;

pub fn phase_address(phase: usize) -> u64 {
    PHASE_PAGES + (phase as u64) * PAGE as u64
}
pub fn row_address(phase: usize, row: usize) -> u64 {
    phase_address(phase) + (ROWS + 192 * row) as u64
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Role {
    pub upgraded: bool,
    pub agility_adjusted: bool,
    pub suppress_general: bool,
    pub high_stance_cost: u32,
    pub effect: Option<ActiveArts>,
}
impl Role {
    fn bytes(self) -> [u8; 24] {
        let mut out = [0; 24];
        out[..4].copy_from_slice(&[
            2,
            self.upgraded.into(),
            self.agility_adjusted.into(),
            self.suppress_general.into(),
        ]);
        out[4..8].copy_from_slice(&self.high_stance_cost.to_le_bytes());
        if let Some(arts) = self.effect {
            out[8] = 1;
            out[9] = arts.ki_phase.encoded();
            out[12..20].copy_from_slice(&arts.distinct_keys.to_le_bytes());
            out[20..24].copy_from_slice(&arts.owner_additive_bits.to_le_bytes());
        }
        out
    }
    pub(crate) fn decode(bytes: [u8; 24]) -> Result<Self, &'static str> {
        let [version @ 1..=2, upgraded @ 0..=1, agility @ 0..=1, suppress @ 0..=1, a, b, c, d, active @ 0..=1, phase @ 0..=2, 0, 0, ..] =
            bytes
        else {
            return Err("invalid Water Sword character inputs");
        };
        if version == 1 && active == 1 {
            return Err("legacy active Arts Proficiency context lacks the Ki charge phase; refresh the character");
        }
        if active == 0 && phase != 0 {
            return Err("inactive Arts Proficiency has a Ki charge phase");
        }
        Ok(Self {
            upgraded: upgraded != 0,
            agility_adjusted: agility != 0,
            suppress_general: suppress != 0,
            high_stance_cost: u32::from_le_bytes([a, b, c, d]),
            effect: (active != 0).then(|| ActiveArts {
                distinct_keys: u64::from_le_bytes(bytes[12..20].try_into().unwrap()),
                owner_additive_bits: u32::from_le_bytes(bytes[20..].try_into().unwrap()),
                ki_phase: KiPhase::from_object(phase.into()),
            }),
        })
    }
    pub(crate) fn effect_words(self, stat: &[u8]) -> [u32; 3] {
        let Some(arts) = self.effect else {
            return [0; 3];
        };
        arts.packet_words(stat, true)
    }
}

impl ActiveArts {
    pub(crate) fn packet_words(self, stat: &[u8], martial: bool) -> [u32; 3] {
        let x = i32::from_le_bytes(stat[0x1bc..0x1c0].try_into().unwrap());
        let points = [
            (10, 1.0f32),
            (200, 1.1),
            (400, 1.2),
            (600, 1.25),
            (800, 1.3),
        ];
        let i = points.partition_point(|(at, _)| *at <= x);
        let curve = match i {
            0 => points[0].1,
            5 => points[4].1,
            _ => {
                let (a, left) = points[i - 1];
                let (b, right) = points[i];
                left + (x - a) as f32 / (b - a) as f32 * (right - left)
            }
        };
        let unit = 10.0f32 * 0.01;
        let keys = if martial { self.distinct_keys } else { 0 };
        let power = unit + (unit * keys as f32).min(50.0f32 * 0.01);
        [
            (curve - 1.0 + power).to_bits(),
            u32::MAX,
            (-1.0f32).to_bits(),
        ]
    }
}

/// Append immutable controller records before hashing the private character context.
pub fn install_controllers(
    image: &mut Vec<u8>,
    known: &mut Vec<u8>,
    role: Role,
) -> Result<(), &'static str> {
    let mut pages = Vec::new();
    let mut masks = Vec::new();
    for (index, digest) in PARAMETERS.iter().enumerate() {
        let state = ActionController::resolve(RESOURCE, FIRST + index as u16, digest)?;
        let selected: Vec<_> = state
            .collision_routes(CollisionComponent::ValueAt80(0))
            .enumerate()
            .filter(|(_, result)| *result == CollisionRoute::Continue)
            .map(|(row, _)| row)
            .collect();
        if selected != [0, 1] {
            return Err("Water Sword recipient selects different rows");
        }
        let base = phase_address(index);
        let mut data = vec![0u8; PAGE];
        let mut mask = vec![0u8; PAGE / 8];
        let mut put = |at: usize, raw: &[u8]| {
            data[at..at + raw.len()].copy_from_slice(raw);
            for i in at..at + raw.len() {
                mask[i / 8] |= 1 << (i % 8);
            }
        };
        put(0x20, &(base + 0x200).to_le_bytes());
        put(0x40, &(base + 0x1000).to_le_bytes());
        put(0x48, &state.first_index.to_le_bytes());
        put(0x4a, &(state.rows.len() as u16).to_le_bytes());
        if index == 0 {
            put(0x80, &role.bytes());
        }
        put(0x200, state.parameter);
        for (i, row) in state.rows.iter().enumerate() {
            put(
                0x1000 + 8 * (usize::from(state.first_index) + i),
                &row_address(index, i).to_le_bytes(),
            );
            put(ROWS + i * 192, row.as_slice());
        }
        pages.extend(base.to_le_bytes());
        pages.extend(data);
        masks.extend(base.to_le_bytes());
        masks.extend(mask);
    }
    // Both files have the same ascending page order, including module pages above this range.
    let stride = PAGE + 8;
    let header = |p: &[u8]| u64::from_le_bytes(p[..8].try_into().unwrap());
    let at = image
        .chunks_exact(stride)
        .take_while(|p| header(p) < PHASE_PAGES)
        .count();
    if image
        .chunks_exact(stride)
        .nth(at)
        .is_some_and(|p| header(p) < phase_address(PARAMETERS.len()))
    {
        return Err("held controller image overlaps a retained page");
    }
    image.splice(at * stride..at * stride, pages);
    let off = at * (PAGE / 8 + 8);
    known.splice(off..off, masks);
    Ok(())
}

/// Positive Ki admits the next sustained action. The automatically emitted
/// finisher has a separate debit even when the last sustain overdraws.
pub fn walk(
    stat: &[u8],
    d: &[u8],
    role: Role,
    base: &[f32; 10],
    after_debit: impl FnMut(usize, f32) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    walk_to(Objective::UntilDepleted, stat, d, role, base, after_debit)
}

pub fn walk_to(
    objective: Objective,
    stat: &[u8],
    d: &[u8],
    role: Role,
    base: &[f32; 10],
    mut after_debit: impl FnMut(usize, f32) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    objective.validate(role)?;
    if stat.len() < 0x7a0 || d.len() < 0x814 {
        return Err("incomplete candidate Ki inputs");
    }
    let word = |bank: &[u8], i: usize| u32::from_le_bytes(bank[i..i + 4].try_into().unwrap());
    let f = |i| f32::from_bits(word(d, i));
    let mut ki = word(stat, 0x1c) as f32;
    if ki <= 0.0 {
        return Err("no full Ki for held attack");
    }
    if d[0x414] != 0
        || d[0x41c] != 0
        || d[0x41d] != 0
        || d[0x8b] != 0
        || f(0x2a8) > 0.0
        || (word(d, 0x2ac) as i32) >= 0
    {
        return Err("held cost requires an additional candidate Ki modifier");
    }
    let general = if role.suppress_general { 0.0 } else { f(0x160) };
    let mut factor = 1.0 - (general + f(0x164));
    if f(0x7f0) > 0.0 {
        factor *= 1.0 - f(0x7f0);
    }
    factor *= (1.0 + f(0x810)) * (1.0 - f(0x170));
    factor *= 1.0 - f(0x570);
    if d[0x6d0] != 0 {
        factor *= 1.0 - 20.0f32 * 0.01;
    }
    if role.agility_adjusted {
        let agility = word(stat, 0x79c);
        let bits = if d[0x6d1] != 0 && agility >= 85 {
            0x3f170a3d
        } else if agility >= 75 {
            0x3f2e147b
        } else if agility >= 25 {
            0x3f3851eb
        } else if agility > 0 {
            0x3f733333
        } else {
            0x40100000
        };
        factor *= f32::from_bits(bits);
    }
    let owner_factor = f32::from_bits(role.high_stance_cost);
    factor *= role
        .effect
        .map_or(owner_factor, |arts| arts.owner_ki_factor(owner_factor));
    if !factor.is_finite() || factor <= 0.0 {
        return Err("held action has no finite Ki depletion");
    }
    let mut phase = 0usize;
    let mut returned = false;
    let exempt = role
        .effect
        .is_some_and(|arts| arts.ki_phase != KiPhase::Charging);
    let mut completed = 0usize;
    loop {
        let next_ki = if exempt {
            ki
        } else {
            ki - base[phase] * factor
        };
        if !next_ki.is_finite() || (!exempt && next_ki >= ki) {
            return Err("held cost does not deplete Ki");
        }
        after_debit(phase, next_ki)?;
        ki = next_ki;
        completed += 1;
        if phase == 9 || (objective == Objective::OpeningThreeActions && completed == 3) {
            return Ok(());
        }
        phase = if ki <= 0.0 {
            9
        } else {
            match phase {
                2 if !role.upgraded => 1,
                6 if !returned => {
                    returned = true;
                    5
                }
                8 => 7,
                _ => phase + 1,
            }
        };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn role(upgraded: bool) -> Role {
        Role {
            upgraded,
            agility_adjusted: false,
            suppress_general: false,
            high_stance_cost: 1.0f32.to_bits(),
            effect: None,
        }
    }
    #[test]
    fn positive_ki_admits_overdraw_then_one_finisher() {
        let mut stat = vec![0; 0x900];
        let mut d = vec![0; 0xb00];
        d[0x2ac..0x2b0].fill(0xff);
        let base = [200., 70., 80., 70., 80., 70., 80., 70., 80., 200.];
        for (budget, upgraded, expected) in [
            (200, true, vec![0, 9]),
            (275, true, vec![0, 1, 2, 9]),
            (490, true, vec![0, 1, 2, 3, 4, 9]),
            (490, false, vec![0, 1, 2, 1, 2, 9]),
            (860, true, vec![0, 1, 2, 3, 4, 5, 6, 5, 6, 7, 9]),
        ] {
            stat[0x1c..0x20].copy_from_slice(&u32::to_le_bytes(budget));
            let mut observed = Vec::new();
            walk(&stat, &d, role(upgraded), &base, |p, _| {
                observed.push(p);
                Ok(())
            })
            .unwrap();
            assert_eq!(observed, expected);
        }
    }

    #[test]
    fn native_debits_and_finish_match_for_both_equipment_reductions() {
        #[derive(Deserialize)]
        struct Case {
            melee: bool,
            martial: bool,
            contacts: usize,
            steps: Vec<(usize, u32)>,
        }
        #[derive(Deserialize)]
        struct Corpus {
            stat_words: Vec<(usize, u32)>,
            d_words: Vec<(usize, u32)>,
            base: [f32; 10],
            cases: Vec<Case>,
        }
        let data: Corpus = serde_json::from_str(include_str!(
            "../tests/fixtures/water_sword_costs_pc202.json"
        )).unwrap();
        let mut stat = [0; 0x900];
        let mut damage = [0; 0xb00];
        for (at, bits) in data.stat_words {
            stat[at..at+4].copy_from_slice(&bits.to_le_bytes());
        }
        for (at, bits) in data.d_words {
            damage[at..at+4].copy_from_slice(&bits.to_le_bytes());
        }
        let actor = Role { agility_adjusted: true, ..role(true) };
        for case in data.cases {
            let mut bank = damage;
            if !case.melee { bank[0x160..0x164].fill(0); }
            if !case.martial { bank[0x170..0x174].fill(0); }
            let mut actual = Vec::new();
            walk(&stat, &bank, actor, &data.base, |phase, remaining| {
                actual.push((phase, remaining.to_bits()));
                Ok(())
            }).unwrap();
            assert_eq!(actual, case.steps);
            assert_eq!(2 * actual.len(), case.contacts);
        }
    }
    #[test]
    fn native_object_packet_preserves_percent_rounding_and_candidate_attribute() {
        let mut stat = vec![0; 0x900];
        let mut r = role(true);
        stat[0x1bc..0x1c0].copy_from_slice(&10i32.to_le_bytes());
        assert_eq!(r.effect_words(&stat), [0; 3]);
        for (count, bits) in [(0, 0x3dcccccc), (1, 0x3e4ccccc), (2, 0x3e999999)] {
            r.effect = Some(ActiveArts {
                distinct_keys: count,
                owner_additive_bits: 0,
                ki_phase: KiPhase::Charging,
            });
            assert_eq!(r.effect_words(&stat), [bits, u32::MAX, (-1.0f32).to_bits()]);
        }
        stat[0x1bc..0x1c0].copy_from_slice(&800i32.to_le_bytes());
        assert!(f32::from_bits(r.effect_words(&stat)[0]) > 0.59);
        assert_eq!(Role::decode(r.bytes()), Ok(r));
    }

    #[test]
    fn free_openings_and_paid_chains_have_different_endpoints() {
        let mut stat = vec![0; 0x900];
        stat[0x1c..0x20].copy_from_slice(&150u32.to_le_bytes());
        let mut d = vec![0; 0xb00];
        d[0x2ac..0x2b0].fill(0xff);
        let mut role = role(true);
        for phase in [KiPhase::Ready, KiPhase::Debited, KiPhase::Charging] {
            role.effect = Some(ActiveArts {
                distinct_keys: 0,
                owner_additive_bits: 0,
                ki_phase: phase,
            });
            assert_eq!(Role::decode(role.bytes()), Ok(role));
            let mut legacy = role.bytes();
            legacy[0] = 1;
            assert!(Role::decode(legacy).is_err());
            let mut debits = Vec::new();
            walk_to(
                Objective::OpeningThreeActions,
                &stat,
                &d,
                role,
                &[100.0; 10],
                |p, k| {
                    debits.push((p, k));
                    Ok(())
                },
            )
            .unwrap();
            if phase == KiPhase::Charging {
                assert_eq!(debits, [(0, 80.), (1, 10.), (2, -60.)]);
                let mut whole = Vec::new();
                walk(&stat, &d, role, &[100.0; 10], |p, k| {
                    whole.push((p, k));
                    Ok(())
                })
                .unwrap();
                assert_eq!(whole, [(0, 80.), (1, 10.), (2, -60.), (9, -130.)]);
            } else {
                assert_eq!(debits, [(0, 150.), (1, 150.), (2, 150.)]);
                assert_eq!(
                    walk(&stat, &d, role, &[100.0; 10], |_, _| panic!(
                        "unbounded cast emitted a contact"
                    )),
                    Err("ARTS_KI_DOES_NOT_DEPLETE")
                );
            }
        }
        role.effect = None;
        let mut legacy = role.bytes();
        legacy[0] = 1;
        assert_eq!(Role::decode(legacy), Ok(role));
    }
}
