//! Character inputs to the writes the compiled formula reads: the Rust form of state_predict.py's _pipeline and _evaluate.
//!
//! Stage order: weapon records (818248), body (2FB2BC), slot 15 (81744C), set rewards (3FAA98), fixed layers, load factor, materializer
//! (819400) and the baseline-frame writes. Anything the Python model refuses returns Err; pipeline_check.py proves the two agree.

use std::collections::HashMap;

#[cfg(test)]
#[path = "pipeline_audit_tests.rs"]
mod audit_tests;
#[cfg(test)]
#[path = "producer_audit_tests.rs"]
mod producer_audit_tests;
#[cfg(test)]
#[path = "source_rebuild_tests.rs"]
mod source_rebuild_tests;
#[cfg(test)]
pub(crate) fn audit_source_program() -> source_rebuild::Program {
    source_rebuild_tests::fixture().3
}
#[cfg(test)]
pub(crate) fn audit_source_character() -> Character {
    source_rebuild_tests::fixture().2
}

pub type R<T> = Result<T, &'static str>;

/// The ordinary source group that lowers a set requirement by one piece.
pub const SET_REDUCTION_GROUP: u16 = 0x38BA;

#[path = "source_dependency.rs"]
pub(crate) mod source_dependency;
#[path = "source_numeric.rs"]
pub mod source_numeric;
#[path = "source_rebuild.rs"]
pub mod source_rebuild;

const ITEM: usize = 0xE8;
const BANK: usize = 0xF70;
const LEGACY_BASE_DEFENSE_SOURCES: u32 = 312 + 66;
const BODY_FLOATS: [(usize, usize, bool); 12] = [
    (0x77c, 0x13a, false),
    (0x780, 0x13c, false),
    (0x784, 0x140, false),
    (0x788, 0x13e, false),
    (0x78c, 0x142, false),
    (0x790, 0x144, false),
    (0x4dc, 0x146, true),
    (0x4fc, 0x148, true),
    (0x51c, 0x14c, true),
    (0x53c, 0x14a, true),
    (0x55c, 0x14e, true),
    (0x57c, 0x150, true),
];
const ACTIVE_CATEGORY: [u16; 16] = [
    0x5197, 0x78F4, 0xC811, 0x9002, 0xBABC, 0xCC71, 0x437D, 0x6670, 0xA335, 0x36F4, 0x5494, 0x4674,
    0x0AC0, 0xC86A, 0xB93B, 0x94E2,
];
const FIXED_CATEGORY: [(u16, u32); 11] = [
    (0xF5E5, 0x1909),
    (0x6449, 0x6E73),
    (0x20FF, 0x5095),
    (0x4420, 0x1C17),
    (0x7F23, 0x2D3F),
    (0xD081, 0x72B1),
    (0x04BE, 0x5E1B),
    (0x5F3A, 0x027C),
    (0x1AE7, 0x0D2F),
    (0x763B, 0x17D6),
    (0xAF69, 0x04E6),
];

#[inline]
fn u16_at(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
pub(crate) fn u32_at(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
#[inline]
fn i32_at(b: &[u8], o: usize) -> i32 {
    u32_at(b, o) as i32
}
#[inline]
fn f32_at(b: &[u8], o: usize) -> f32 {
    f32::from_bits(u32_at(b, o))
}
#[inline]
fn put_u32(b: &mut [u8], o: usize, v: u32) {
    b[o..o + 4].copy_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_u64(b: &mut [u8], o: usize, v: u64) {
    b[o..o + 8].copy_from_slice(&v.to_le_bytes());
}
#[inline]
fn put_f32(b: &mut [u8], o: usize, v: f32) {
    put_u32(b, o, v.to_bits());
}

/// CVTTSS2SI: truncates, and gives the indefinite value out of range.
#[inline]
fn cvtt32(x: f32) -> i32 {
    if x.is_finite() && (-2147483648.0..2147483648.0).contains(&x) {
        x as i32
    } else {
        i32::MIN
    }
}
#[inline]
fn cvtt64(x: f32) -> i64 {
    if x.is_finite() && (-9223372036854775808.0..9223372036854775808.0).contains(&x) {
        x as i64
    } else {
        i64::MIN
    }
}
#[inline]
fn finite(x: f32) -> R<f32> {
    if x.is_finite() {
        Ok(x)
    } else {
        Err("nonfinite value outside the finite domain")
    }
}

/// Named byte sections: u32 name length, name, u32 size, bytes.
struct Section {
    offset: usize,
    bytes: Vec<u8>,
}

struct Sections(HashMap<String, Section>);

impl Sections {
    fn parse(data: &[u8]) -> R<Sections> {
        let (mut out, mut at) = (HashMap::new(), 0usize);
        let len = |at: usize| -> R<usize> {
            data.get(at..at + 4)
                .map(|b| u32_at(b, 0) as usize)
                .ok_or("section header past the end")
        };
        while at < data.len() {
            let n = len(at)?;
            let name = data
                .get(at + 4..at + 4 + n)
                .and_then(|b| std::str::from_utf8(b).ok())
                .ok_or("bad section name")?;
            at += 4 + n;
            let size = len(at)?;
            let body = data
                .get(at + 4..at + 4 + size)
                .ok_or("section past the end")?;
            let section = Section {
                offset: at + 4,
                bytes: body.to_vec(),
            };
            if out.insert(name.to_string(), section).is_some() {
                return Err("duplicate named section");
            }
            at += 4 + size;
        }
        Ok(Sections(out))
    }

    fn get(&self, name: &str) -> R<&Vec<u8>> {
        self.0
            .get(name)
            .map(|section| &section.bytes)
            .ok_or("missing section")
    }
}

fn words(b: &[u8]) -> Vec<u32> {
    b.as_chunks::<4>()
        .0
        .iter()
        .map(|c| u32::from_le_bytes(*c))
        .collect()
}

/// A native table: header-prefixed rows and its key to index map.
struct Keyed {
    rows: Vec<u8>,
    stride: usize,
    direct: Vec<u32>,
    sparse: Vec<(u32, u32)>,
}

const ABSENT: u32 = u32::MAX;

impl Keyed {
    fn new(s: &mut Sections, name: &str, stride: usize) -> R<Keyed> {
        let rows =
            s.0.remove(&format!("{name}.rows"))
                .map(|section| section.bytes)
                .ok_or("missing table rows")?;
        let w = words(s.get(&format!("{name}.keys"))?);
        let mut pairs: Vec<(u32, u32)> =
            w.as_chunks::<2>().0.iter().map(|p| (p[0], p[1])).collect();
        pairs.sort_unstable();
        let mut direct = Vec::new();
        if pairs.last().map_or(0, |p| p.0) < 1 << 20 {
            direct = vec![ABSENT; pairs.last().map_or(0, |p| p.0 as usize + 1)];
            for &(k, i) in &pairs {
                direct[k as usize] = i;
            }
            pairs.clear();
        }
        let widest = pairs
            .iter()
            .map(|p| p.1)
            .chain(direct.iter().copied().filter(|&i| i != ABSENT))
            .max();
        if widest.is_some_and(|i| 8 + (i as usize + 1) * stride > rows.len()) {
            return Err("table key past its rows");
        }
        Ok(Keyed {
            rows,
            stride,
            direct,
            sparse: pairs,
        })
    }

    #[inline]
    fn index(&self, key: u32) -> Option<u32> {
        if self.sparse.is_empty() {
            match self.direct.get(key as usize) {
                Some(&i) if i != ABSENT => Some(i),
                _ => None,
            }
        } else {
            self.sparse
                .binary_search_by_key(&key, |p| p.0)
                .ok()
                .map(|i| self.sparse[i].1)
        }
    }

    #[inline]
    fn row(&self, key: u32) -> Option<&[u8]> {
        self.index(key)
            .map(|i| &self.rows[8 + i as usize * self.stride..8 + (i as usize + 1) * self.stride])
    }
}

#[derive(Clone, Copy)]
struct Recipe {
    kind: u8,
    offset: usize,
    width: usize,
    scale: Option<f32>,
    literal: u64,
}

// recipe kinds
const LITERAL: u8 = 0;
const D_SET: u8 = 1;
const D_ADD: u8 = 2;
const D_OLD_MAX: u8 = 3;
const D_MAX: u8 = 4;
const D_OLD_V: u8 = 5;
const STAT_ADD_F: u8 = 6;
const STAT_ADD_U16: u8 = 7;
const STAT_ADD_U32: u8 = 8;

// group classes
const NO_WRITE: u8 = 1;
const RECIPES: u8 = 2;
const STATEFUL: u8 = 3;
const CONDITIONAL: u8 = 4;

#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    pub values: Vec<(u16, u64)>,
    pub sides: Vec<u32>,
    pub weight: i64,
    pub level: u32,
    pub flag: bool,
    /// Exact item defense when the evaluator can describe it; unknown never dominates.
    pub defense: Option<u64>,
}

pub struct Tables {
    source_digest: String,
    items: Keyed,
    effects: Keyed,
    groups: Keyed,
    rarity: Keyed,
    sides: Keyed,
    special: Keyed,
    core300: Keyed,
    core340: Keyed,
    level_rows: Vec<u8>,
    base70: Vec<u8>,
    power90: Vec<u8>,
    base90: Vec<u8>,
    fam_steps: usize,
    fam_bonus_steps: usize,
    fam: Vec<u32>,
    alternate: Vec<(u32, u32)>,
    alternate_default: usize,
    grade_rows: Vec<u8>,
    grade_index: Vec<(u32, u32)>,
    class: Vec<(u8, u32, u32)>,
    recipes: Vec<Recipe>,
    families: [i32; 2],
    slots5: [i32; 5],
    slots8: [i32; 8],
    c_percent: f32,
    c_4688: f32,
    c_one: f32,
    c_half: f32,
    c_tenth: f32,
    c_atten: f32,
    c_4894: f32,
    c_4080: f32,
    opt_recovery: f32,
    opt_e40f: f32,
    opt_22af: f32,
    opt_1f32: f32,
    opt_d1f: f32,
    opt_dc22: f32,
    opt_d84f: f32,
    aeac_int: i32,
}

#[derive(Clone, Copy, Debug)]
pub struct Entry {
    pub key: u16,
    pub base: u32,
    pub dup: u16,
}

pub struct Source {
    mode: i32,
    thresholds: [u32; 7],
    pub maps: [Vec<Entry>; 2],
    sides: [Vec<[u8; 68]>; 2],
    // Snapshot the reduction at the native set-reward stage, before fixed
    // profile operations can change the final source map.
    set_reduction: [bool; 2],
    set_pieces: [Vec<(u32, usize)>; 2],
    #[cfg(test)]
    pub(crate) set_row_activations: Vec<SetRowActivation>,
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub(crate) struct SetRowActivation {
    pub(crate) side: u32,
    pub(crate) bank: usize,
    pub(crate) row_index: usize,
    pub(crate) threshold: u32,
    pub(crate) reward_key: u16,
    pub(crate) flags: u8,
}

/// Qualified native set membership, including retained and reserve equipment.
/// Counts come from side_entry, which applies fixed-set item deduplication.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SetState {
    pub pieces: Vec<(u32, usize)>,
    pub reduction: bool,
}

/// A componentwise optimistic bound. Qualification, fixed identities and
/// collector capacity can reduce the native count.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct SetCapacity {
    pub(crate) pieces: Vec<usize>,
    pub(crate) reduction: bool,
}

impl SetCapacity {
    pub(crate) fn empty(keys: &[u32]) -> Self {
        Self {
            pieces: vec![0; keys.len()],
            reduction: false,
        }
    }
}

impl Source {
    pub(crate) fn set_state(&self) -> SetState {
        let bank = selected_map(self.mode, self.mode);
        SetState {
            pieces: self.set_pieces[bank].clone(),
            reduction: self.set_reduction[bank],
        }
    }
}

/// Explicit profile deltas are not a substitute for the native source
/// collector. Preserve duplicate-count residue when a negative delta cancels
/// only the base component of an entry.
fn apply_operations(entries: &mut Vec<Entry>, ops: &[(u16, i64)]) -> R<()> {
    for &(group, value) in ops {
        match entries.iter().position(|e| e.key == group) {
            Some(i) => {
                entries[i].base = entries[i].base.wrapping_add(value as u32);
                if value < 0 && entries[i].base == 0 && entries[i].dup == 0 {
                    entries.remove(i);
                }
            }
            None => entries.push(Entry {
                key: group,
                base: value as u32,
                dup: 0,
            }),
        }
    }
    if entries.len() > 256 {
        return Err("aggregate exceeds its native capacity");
    }
    Ok(())
}

/// Highest item level the formula reads; the blacksmith's soul match raises gear up to it.
pub const LEVEL_CAP: u16 = 180;

/// One worn state: the capture's inventory and the character facts that do not change with attributes.
#[derive(Clone)]
pub struct Character {
    pub inv: Vec<u8>,
    pub mode: i32,
    pub active: usize,
    cap: u32,
    plus_cap: u32,
    style: i32,
    unarmed: u32,
    level: u32,
    core_mode: i32,
    core_maps: [Vec<Entry>; 2],
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum PacketKind {
    Weapon,
    Fixed(u16),
    DamageRow(i8),
}

impl PacketKind {
    pub(crate) fn resolve(self, weapon_kind: u32) -> u32 {
        match self {
            Self::Weapon => weapon_kind,
            Self::Fixed(kind) => u32::from(kind),
            Self::DamageRow(1) if weapon_kind == 2 => 2,
            Self::DamageRow(kind) => kind as i32 as u32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HitPacket {
    Prepared,
    /// The projectile restores its own imbue fields and retains candidate attack.
    UnimbuedOwnerProjectile { power: u32, main_flags: u32 },
}

/// The declared scene baseline every prediction lands on.
pub struct Frame {
    stat: Vec<u8>,
    d0: Vec<u8>,
    s_base: Vec<u8>,
    d_base: Vec<u8>,
    d_base_offset: usize,
    main8: u32,
    pub(crate) packet_kind: PacketKind,
    pub(crate) hit_packets: Vec<HitPacket>,
    /// Context-owned values over the exact declared attack row/address layout.
    pub(crate) runtime_hit_inputs: Option<Vec<u8>>,
    weapon_element_packet: bool,
    stat_addr: u64,
    d_addr: u64,
    main_addr: u64,
    ext_addr: u64,
    owner: u64,
    /// Native2FEBFC input owned by the source actor, not the frozen target.
    owner_resource: Option<(u32, i32)>,
    actor_agility: Option<[u8; 4]>,
    native_before: Vec<(u16, u32)>,
    native_after: Vec<(u16, u32)>,
    pub(crate) source_rebuild: Option<source_rebuild::Program>,
    source_baseline_ready: bool,
    attr: u64,
    dest: u64,
    hp_base: u32,
    baseline_defense_sources: u32,
    pub(crate) held_reaper: bool,
    /// Explicit original376698 zero-descriptor/static-resource contract.
    pub(crate) identity_ki_seed: bool,
}

pub struct Scratch {
    pub stat: Vec<u8>,
    pub d: Vec<u8>,
    pub(crate) pred: Vec<u8>,
    pub source: Source,
    pub writes: Vec<(u64, u32)>,
    transformed: [i32; 7],
    pub source_rebuild_trace: Option<source_rebuild::Trace>,
    pub(crate) legacy_source_signature: Option<source_dependency::Signature>,
}

impl Scratch {
    pub fn new() -> Scratch {
        Scratch {
            stat: vec![0; 0x900],
            d: vec![0; 0xB00],
            pred: vec![0; 0x900],
            source: Source {
                mode: 0,
                thresholds: [0; 7],
                maps: [Vec::with_capacity(300), Vec::with_capacity(300)],
                sides: [Vec::with_capacity(20), Vec::with_capacity(20)],
                set_reduction: [false; 2],
                set_pieces: [Vec::new(), Vec::new()],
                #[cfg(test)]
                set_row_activations: Vec::new(),
            },
            writes: Vec::with_capacity(0x800),
            transformed: [0; 7],
            source_rebuild_trace: None,
            legacy_source_signature: None,
        }
    }
}

impl Default for Scratch {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone, Copy)]
struct Effective {
    level: u32,
    plus: u32,
}

impl Tables {
    pub fn load(data: &[u8]) -> R<Tables> {
        let mut s = Sections::parse(data)?;
        let items = Keyed::new(&mut s, "item", 0x1A0)?;
        let effects = Keyed::new(&mut s, "effect", 0xD8)?;
        let groups = Keyed::new(&mut s, "group", 0x70)?;
        let rarity = Keyed::new(&mut s, "rarity", 0xF8)?;
        let sides = Keyed::new(&mut s, "side", 0xD8)?;
        let special = Keyed::new(&mut s, "special", 0x1C)?;
        let core300 = Keyed::new(&mut s, "core300", 0x80)?;
        let core340 = Keyed::new(&mut s, "core340", 0x44)?;
        let fam = words(s.get("fam")?);
        let alt = words(s.get("alternate")?);
        let c = words(s.get("consts")?);
        let f = |i: usize| f32::from_bits(c[i]);
        let grade_words = words(s.get("grade.index")?);
        let mut grade_index = vec![(0, 0); 65536];
        for r in grade_words.as_chunks::<3>().0 {
            *grade_index
                .get_mut(r[0] as usize)
                .ok_or("grade key past 16 bits")? = (r[1], r[2]);
        }
        let mut class = vec![(0u8, 0, 0); 65536];
        let mut recipes = Vec::new();
        let m = words(s.get("materializer")?);
        let mut at = 0;
        while at < m.len() {
            let [group, kind, n] = *m
                .get(at..at + 3)
                .and_then(|h| <&[u32; 3]>::try_from(h).ok())
                .ok_or("short materializer header")?;
            let (group, kind, n) = (group as usize, kind as u8, n as usize);
            at += 3;
            *class
                .get_mut(group)
                .ok_or("materializer group past 16 bits")? = (kind, recipes.len() as u32, n as u32);
            for r in m
                .get(at..at + 7 * n)
                .ok_or("short materializer recipes")?
                .as_chunks::<7>()
                .0
            {
                recipes.push(Recipe {
                    kind: r[0] as u8,
                    offset: r[1] as usize,
                    width: r[2] as usize,
                    scale: (r[3] != 0).then(|| f32::from_bits(r[4])),
                    literal: r[5] as u64 | (r[6] as u64) << 32,
                });
            }
            at += 7 * n;
        }
        let i32s = |name: &str| -> R<Vec<i32>> {
            Ok(words(s.get(name)?).iter().map(|&v| v as i32).collect())
        };
        if fam.len() < 2 || alt.is_empty() || c.len() < 16 {
            return Err("short constant section");
        }
        Ok(Tables {
            source_digest: {
                use sha2::{Digest, Sha256};
                format!("{:x}", Sha256::digest(data))
            },
            items,
            effects,
            groups,
            rarity,
            sides,
            special,
            core300,
            core340,
            level_rows: s.get("level")?.clone(),
            base70: s.get("base70")?.clone(),
            power90: s.get("power90")?.clone(),
            base90: s.get("base90")?.clone(),
            fam_steps: fam[0] as usize,
            fam_bonus_steps: fam[1] as usize,
            fam: fam[2..].to_vec(),
            alternate_default: alt[0] as usize,
            alternate: alt[1..]
                .as_chunks::<2>()
                .0
                .iter()
                .map(|p| (p[0], p[1]))
                .collect(),
            grade_rows: s.get("grade.rows")?.clone(),
            grade_index,
            class,
            recipes,
            families: i32s("families")?
                .try_into()
                .map_err(|_| "families needs two words")?,
            slots5: i32s("slots5")?
                .try_into()
                .map_err(|_| "slots5 needs five words")?,
            slots8: i32s("slots8")?
                .try_into()
                .map_err(|_| "slots8 needs eight words")?,
            c_percent: f(0),
            c_4688: f(1),
            c_one: f(2),
            c_half: f(3),
            c_tenth: f(4),
            c_atten: f(5),
            c_4894: f(6),
            c_4080: f(7),
            opt_recovery: f(8),
            opt_e40f: f(9),
            opt_22af: f(10),
            opt_1f32: f(11),
            opt_d1f: f(12),
            opt_dc22: f(13),
            opt_d84f: f(14),
            aeac_int: c[15] as i32,
        })
    }

    #[inline]
    fn field70(&self, index: u32, off: usize) -> u32 {
        let idx = (index as usize).min(self.base70.len() / 0x94 - 1);
        u16_at(&self.base70, idx * 0x94 + off) as u32
    }
    pub fn input_digest(&self) -> &str {
        &self.source_digest
    }
    pub fn unarmed_attack(&self, levels: [u32; 2], plus_cap: u32) -> u32 {
        (self.field70(levels[0].min(190), 0x22) + self.field70(levels[1].min(plus_cap), 0x24))
            & 0xffff
    }

    #[inline]
    fn field90(&self, index: i32, off: usize) -> u32 {
        u16_at(&self.power90, index.clamp(1, 999) as usize * 32 + off) as u32
    }

    fn familiarity(&self, raw: u32, rarity: u8) -> R<u32> {
        if rarity > 5 {
            return Ok(0);
        }
        let r = rarity as usize;
        let thresholds = &self.fam[r * self.fam_steps..(r + 1) * self.fam_steps];
        let steps = thresholds.iter().take_while(|&&t| raw >= t).count();
        if steps == 0 {
            return Ok(0);
        }
        if steps > self.fam_bonus_steps {
            return Err("familiarity step past the bonus rows");
        }
        Ok(self.fam[6 * self.fam_steps + r * self.fam_bonus_steps + steps - 1])
    }

    fn effective(&self, cap: u32, item: &[u8], familiarity: bool) -> R<Effective> {
        let rarity = item[0x30];
        let flags = u32_at(item, 0x18);
        let base = if flags & 0x200000 != 0 {
            0
        } else {
            (u16_at(item, 6) as u32).min(180)
        };
        let fam = if familiarity {
            self.familiarity(u32_at(item, 0x14), rarity)?
        } else {
            0
        };
        let mut plus = (u16_at(item, 0xA) as u32).min(cap);
        let mut add = 0;
        if rarity <= 5 {
            if let Some(row) = self.rarity.row(rarity as u32) {
                add = u32_at(row, 0x14);
            }
            if flags & 0x40000 == 0 && rarity == 5 {
                plus = plus.wrapping_add(self.aeac_int as u32);
            }
        }
        Ok(Effective {
            level: base.wrapping_add(fam).wrapping_add(add),
            plus,
        })
    }

    fn selection<'a>(&'a self, item: &[u8]) -> R<(&'a [u8], &'a [u8])> {
        let key = u16_at(item, 0) as u32;
        if key == 0 {
            return Err("empty item key");
        }
        let original = self.items.row(key).ok_or("missing item")?;
        let mut selected = key;
        if item[0x30] as i8 >= 4 {
            let redirected = u16_at(original, 0x160) as u32;
            if redirected != 0 {
                selected = redirected;
            }
        }
        Ok((
            original,
            self.items.row(selected).ok_or("missing redirected item")?,
        ))
    }

    fn flat_group(&self, item: &[u8], group: u16) -> u32 {
        let mut total = 0u32;
        for i in 0..7 {
            let effect = u32_at(item, 0x38 + 24 * i);
            if effect == u32::MAX {
                continue;
            }
            if let Some(row) = self.effects.row(effect) {
                if u16_at(row, 2) == group {
                    total = total.wrapping_add(u32_at(item, 0x3C + 24 * i));
                }
            }
        }
        total
    }

    fn requirement_delta(&self, cap: u32, item: &[u8]) -> u32 {
        if (item[0x30] as i8) < 3 {
            return 0;
        }
        let count = (item[0x31] == 1) as i32 + (item[0x32] == 1) as i32;
        let coefficient = count as f32 * self.c_half;
        let plus = (u16_at(item, 0xA) as u32).min(cap);
        cvtt64(self.field70(plus, 0x62) as f32 * coefficient) as u32
    }

    fn requirements(&self, cap: u32, raw: &[i32; 7], original: &[u8], item: &[u8]) -> bool {
        let kind = u32_at(original, 0x60);
        let column = ((item[0x30] as i8) >= 5 && kind != 0x2D32) as usize;
        let delta = self.requirement_delta(cap, item);
        (0..7).all(|i| {
            let b = original[0x18C + i * 2 + column] as u32;
            let needed = if b != 0 {
                b.wrapping_add(delta) as i32
            } else {
                0
            };
            needed <= raw[i]
        })
    }

    /// Selected base, coefficient and level column of a weapon channel.
    fn weapon_fields(&self, item: &[u8], style: i32, secondary: bool) -> R<(i32, i32, usize)> {
        let (original, p) = self.selection(item)?;
        let alt = secondary && u32_at(original, 0x58) != 0;
        let mut base = u32_at(p, if alt { 0x14 } else { 0x10 });
        let mut coefficient = u32_at(p, if alt { 0xD8 } else { 0xD4 });
        let off = match style {
            0 => Some(if alt { 0x20 } else { 0x18 }),
            2 => Some(if alt { 0x38 } else { 0x30 }),
            _ => None,
        };
        if let Some(o) = off {
            base = base.wrapping_add(u32_at(p, o));
            coefficient = coefficient.wrapping_add(u32_at(p, o + 4));
        }
        let mut level_field = 0x1E;
        if alt {
            let selector = u32_at(original, 0x58);
            level_field = self
                .alternate
                .iter()
                .find(|a| a.0 == selector)
                .map_or(self.alternate_default, |a| a.1 as usize);
        }
        Ok((
            (base as i32).max(0),
            (coefficient as i32).max(0),
            level_field,
        ))
    }

    /// Special-category factor and the signed minimum both flag-40000 scalars share.
    fn special_bound(
        &self,
        item: &[u8],
        base: i32,
        reference: i32,
        category: u32,
        slope_at: usize,
        intercept_at: usize,
    ) -> R<i32> {
        if self.special.rows.is_empty() {
            return Err("the manager970 table was not captured");
        }
        let Some(row) = self.special.row(category) else {
            return Ok(base);
        };
        let (slope, intercept) = (f32_at(row, slope_at), f32_at(row, intercept_at));
        if !slope.is_finite() || !intercept.is_finite() {
            return Err("nonfinite manager970 coefficients");
        }
        let flags = u32_at(item, 0x18);
        let level = if flags & 0x200000 != 0 {
            0
        } else {
            (u16_at(item, 6) as u32).min(180)
        };
        let term = level as f32 * (slope * self.c_percent);
        let product = (term + intercept) * reference as f32;
        finite(product)?;
        Ok(base.min(cvtt32(product)))
    }

    fn weapon_scalar(&self, c: &Character, item: &[u8], admitted: bool, secondary: bool) -> R<u32> {
        let (_, selected) = self.selection(item)?;
        let (base, coefficient, level_field) = self.weapon_fields(item, c.style, secondary)?;
        let mut bounded = base;
        if u32_at(item, 0x18) & 0x40000 != 0 {
            let linked_key = u16_at(selected, 0x160) as u32;
            let (mut reference, mut category) = (base, u16_at(selected, 0x152) as u32);
            if let Some(linked) = self.items.row(linked_key) {
                reference = i32_at(linked, 0x10);
                category = u16_at(linked, 0x152) as u32;
            }
            bounded = self.special_bound(item, base, reference, category, 4, 0x10)?;
        }
        let e = self.effective(c.cap, item, true)?;
        let level = e.level.min(190);
        let plus = e.plus.min(c.plus_cap);
        let product = (self.field70(level, level_field) + self.field70(plus, 0x20))
            .wrapping_mul(coefficient as u32);
        let flat = if admitted {
            self.flat_group(item, 0x8459)
        } else {
            0
        };
        Ok((product / 1000)
            .wrapping_add(bounded as u32)
            .wrapping_add(flat))
    }

    fn auxiliary(&self, item: &[u8], style: i32, secondary: bool) -> R<u32> {
        let (original, selected) = self.selection(item)?;
        let (parameter, style0, style2, slope, intercept) = if secondary {
            (0xE0, 0x2C, 0x44, 0xC, 0x18)
        } else {
            (0xDC, 0x28, 0x40, 8, 0x14)
        };
        let mut value = u32_at(selected, parameter);
        if style == 0 {
            value = value.wrapping_add(u32_at(selected, style0));
        } else if style == 2 {
            value = value.wrapping_add(u32_at(selected, style2));
        }
        let base = value as i32;
        let mut bounded = base;
        if u32_at(item, 0x18) & 0x40000 != 0 {
            let linked_key = u16_at(original, 0x160) as u32;
            let (mut reference, mut category) = (base, u16_at(original, 0x152) as u32);
            if let Some(linked) = self.items.row(linked_key) {
                reference = i32_at(linked, parameter);
                category = u16_at(linked, 0x152) as u32;
            }
            bounded = self.special_bound(item, base, reference, category, slope, intercept)?;
        }
        Ok(bounded.max(0) as u32)
    }

    fn default_core(&self, original: &[u8]) -> ([u32; 3], u32) {
        if let Some(row) = self.core300.row(u32_at(original, 0x58)) {
            return (
                [u32_at(row, 0x28), u32_at(row, 0x2C), u32_at(row, 0x30)],
                u32_at(row, 0x34),
            );
        }
        if let Some(row) = self.core340.row(u32_at(original, 0x5C)) {
            return (
                [u32_at(row, 0x1C), u32_at(row, 0x20), u32_at(row, 0x24)],
                u32_at(row, 0x28),
            );
        }
        ([0; 3], 0)
    }

    fn local_mask(&self, item: &[u8]) -> R<u32> {
        let original = self
            .items
            .row(u16_at(item, 0) as u32)
            .ok_or("missing item")?;
        let (_, native) = self.default_core(original);
        Ok(if item[0x33] != 0 {
            item[0x33] as u32
        } else {
            native
        })
    }

    fn core_mask(&self, c: &Character, item: &[u8], original: &[u8], slot: usize) -> R<u32> {
        let mut mask = self.local_mask(item)?;
        let flags = u32_at(original, 0xB0);
        for family in [0usize, 1] {
            if flags & (1 << (11 + family)) == 0 {
                continue;
            }
            let key = if family == 0 { 0xD7BF } else { 0xCB4B };
            let map = &c.core_maps[(c.core_mode == 1) as usize];
            let value = map
                .iter()
                .find(|e| e.key == key)
                .map_or(-1, |e| e.base.wrapping_add(e.dup as u32) as i32);
            if value >= 0 {
                let other = &c.inv[(family ^ 1) * BANK + slot * ITEM..][..ITEM];
                if self.items.index(u16_at(other, 0) as u32).is_some() {
                    mask = self.local_mask(other)?;
                    break;
                }
            }
        }
        Ok(mask)
    }

    fn core_bonus(
        &self,
        c: &Character,
        transformed: &[i32; 7],
        item: &[u8],
        original: &[u8],
        slot: usize,
    ) -> R<u32> {
        let (coefficients, _) = self.default_core(original);
        let mask = self.core_mask(c, item, original, slot)?;
        let mut values = [0i32; 7];
        for j in 0..7 {
            if mask & (1 << j) != 0 {
                values[j] = transformed[j];
            }
        }
        values.sort_by(|a, b| b.cmp(a));
        let mut total = 0u32;
        for k in 0..3 {
            total = total.wrapping_add(coefficients[k].wrapping_mul(values[k] as u32));
        }
        Ok(total / 100)
    }

    fn diminished(&self, base_u32: u32, bonus: u32) -> R<u32> {
        let base = base_u32 as f32;
        let mut remaining = bonus as f32;
        let tranche = base * self.c_tenth;
        let half = base * self.c_half;
        if remaining <= 0.0 {
            return Ok(0);
        }
        if remaining <= half {
            return Ok(cvtt64(remaining + 0.0) as u32);
        }
        remaining -= half;
        let mut acc = half + 0.0;
        for _ in 0..1000 {
            remaining *= self.c_atten;
            if remaining < 1.0 {
                return Ok(cvtt64(acc) as u32);
            }
            if remaining <= tranche {
                acc += remaining;
                return Ok(cvtt64(acc) as u32);
            }
            acc += tranche;
            remaining -= tranche;
        }
        Err("attenuation iteration budget exceeded")
    }

    fn pair(&self, base: u32, bonus: u32) -> R<u32> {
        let summed = base.wrapping_add(self.diminished(base, bonus)?);
        Ok(cvtt64(finite(summed as f32)?) as u32)
    }

    fn side_entry(
        &self,
        entry: &mut [u8; 68],
        item_key: u16,
        level: i32,
        special: u8,
        context: i32,
    ) -> R<()> {
        let parameter = self.sides.row(u32_at(entry, 0)).ok_or("missing side key")?;
        let mut count = u16_at(entry, 4) as u32;
        let fixed = u32_at(parameter, 0x10) != 0;
        if !fixed && count > 7 {
            return Err("side free-slot count above seven");
        }
        let mut levels = [0u16; 7];
        let mut items = [0u16; 7];
        for i in 0..7 {
            levels[i] = u16_at(entry, 0x10 + 2 * i);
            items[i] = u16_at(entry, 0x2C + 2 * i);
        }
        if fixed {
            let Some(slot) = (0..7).find(|&i| u16_at(parameter, 0x14 + 4 * i) == item_key) else {
                return Ok(());
            };
            let old = levels[slot];
            if old == 0 || (old as i32) < level {
                levels[slot] = level as u16;
                items[slot] = item_key;
                if old == 0 {
                    count = (count + 1) & 65535;
                    entry[4..6].copy_from_slice(&(count as u16).to_le_bytes());
                }
                if special != 0 && (old == 0 || entry[0x3A + slot] == 0) {
                    let v = u16_at(entry, 6).wrapping_add(1);
                    entry[6..8].copy_from_slice(&v.to_le_bytes());
                    entry[0x3A + slot] = 1;
                }
            }
        } else {
            let free = (0..7).find(|&i| levels[i] == 0);
            let slot = free.or((0..7).rev().find(|&i| levels[i] > 0));
            if let Some(s) = slot {
                levels[s] = level as u16;
                items[s] = item_key;
                if free.is_some() {
                    count = (count + 1) & 65535;
                    entry[4..6].copy_from_slice(&(count as u16).to_le_bytes());
                }
            }
            if count > 7 {
                return Err("side free-slot count above seven");
            }
            let n = count as usize;
            let mut ranks = [0u32; 7];
            for i in 0..n {
                ranks[i] = self.items.index(items[i] as u32).unwrap_or(0);
            }
            // The native nested swap, with its pass-wide changed flag.
            let mut settled = false;
            for _ in 0..1000 {
                let mut changed = false;
                for left in 0..n {
                    for right in left + 1..n {
                        let better = levels[right] > levels[left]
                            || (levels[right] == levels[left] && ranks[right] < ranks[left]);
                        if better {
                            changed = true;
                        }
                        if changed {
                            levels.swap(left, right);
                            items.swap(left, right);
                            ranks.swap(left, right);
                            break;
                        }
                    }
                }
                if !changed {
                    settled = true;
                    break;
                }
            }
            if !settled {
                return Err("side sort exceeded its budget");
            }
        }
        for i in 0..7 {
            entry[0x10 + 2 * i..0x12 + 2 * i].copy_from_slice(&levels[i].to_le_bytes());
            entry[0x2C + 2 * i..0x2E + 2 * i].copy_from_slice(&items[i].to_le_bytes());
        }
        entry[0xC..0x10].copy_from_slice(&(context as u32).to_le_bytes());
        let mut ordered = [0u16; 7];
        let mut k = 0;
        for &v in levels.iter().filter(|&&v| v > 0) {
            ordered[k] = v;
            k += 1;
        }
        ordered[..k].sort_unstable_by(|a, b| b.cmp(a));
        let mut sum = 0u32;
        for index in 0..7 {
            sum += ordered[index] as u32;
            let value = if (index as u32) < count {
                (sum / (index as u32 + 1)) as u16
            } else {
                u16_at(entry, 0x1C + 2 * index)
            };
            entry[0x1E + 2 * index..0x20 + 2 * index].copy_from_slice(&value.to_le_bytes());
        }
        Ok(())
    }

    fn aggregate(&self, source: &mut Source, context: i32, key: u16, value: u32) {
        if key == 0 {
            return;
        }
        let Some(row) = self.groups.row(key as u32) else {
            return;
        };
        let threshold = match u16_at(row, 0xC) {
            0xF8B5 => Some(0),
            0xECC5 => Some(1),
            0x4329 => Some(2),
            0x3711 => Some(3),
            0x89DE => Some(4),
            0x35BE => Some(5),
            0xB51B => Some(6),
            _ => None,
        };
        if let Some(t) = threshold {
            if value as i32 > source.thresholds[t] as i32 {
                return;
            }
        }
        let bank = selected_map(context, source.mode);
        let entries = &mut source.maps[bank];
        let Some(e) = entries.iter_mut().find(|e| e.key == key) else {
            if entries.len() < 256 {
                entries.push(Entry {
                    key,
                    base: value,
                    dup: 0,
                });
            }
            return;
        };
        if key == 0x38BA {
            e.base = e.base.wrapping_add(e.dup as u32).max(value);
        } else if row[0x50] & 4 != 0 {
            e.base = e.base.max(value);
            e.dup = e.dup.wrapping_add(1);
        } else {
            let d = e.dup as u32;
            e.base = e.base.wrapping_add(d).wrapping_add(value);
            let cap = i32_at(row, 4);
            if cap >= 0 && e.base.wrapping_add(d) as i64 > cap as i64 {
                e.base = cap as u32;
            }
        }
    }

    fn collect(
        &self,
        source: &mut Source,
        item: &[u8],
        reserve: bool,
        context: i32,
        mask: u8,
    ) -> R<()> {
        let key = u16_at(item, 0);
        let Some(parameter) = self.items.row(key as u32) else {
            return Ok(());
        };
        let item_type = parameter[0x182];
        for slot in 0..7 {
            let (effect, value) = (
                u32_at(item, 0x38 + slot * 24),
                u32_at(item, 0x3C + slot * 24),
            );
            if effect == u32::MAX {
                continue;
            }
            let Some(effect_row) = self.effects.row(effect) else {
                continue;
            };
            let group = u16_at(effect_row, 2);
            if group == 0 {
                continue;
            }
            let Some(row) = self.groups.row(group as u32) else {
                continue;
            };
            if !reserve
                && ((item_type == 1 || item_type == 2) && group == 0x8459
                    || item_type == 3 && group == 0xDA05)
            {
                continue;
            }
            let side_key = u32_at(row, 8);
            if side_key != 0 {
                let flags = u32_at(item, 0x18);
                let level = if flags & 0x200000 != 0 {
                    0
                } else {
                    (u16_at(item, 6) as i32).min(180)
                };
                let bank = selected_map(context, source.mode);
                let found = source.sides[bank]
                    .iter()
                    .position(|e| u32_at(e, 0) == side_key);
                if found.is_none() && source.sides[bank].len() == 20 {
                    continue;
                }
                let index = found.unwrap_or_else(|| {
                    let mut e = [0u8; 68];
                    e[..4].copy_from_slice(&side_key.to_le_bytes());
                    source.sides[bank].push(e);
                    source.sides[bank].len() - 1
                });
                self.side_entry(
                    &mut source.sides[bank][index],
                    key,
                    level,
                    ((flags >> 19) & 1) as u8,
                    context,
                )?;
                continue;
            }
            if reserve {
                if u16_at(row, 0xC) != 0x38BA {
                    continue;
                }
            } else if mask & (1 << slot) == 0 {
                continue;
            }
            self.aggregate(source, context, group, value);
        }
        Ok(())
    }

    fn prepared_attack_pair(
        &self,
        c: &Character,
        transformed: &[i32; 7],
        item: &[u8],
        original: &[u8],
        slot: usize,
        admitted: bool,
    ) -> R<(u32, u32)> {
        let primary = self.weapon_scalar(c, item, admitted, false)?;
        let secondary = self.weapon_scalar(c, item, admitted, true)?;
        let core = self.core_bonus(c, transformed, item, original, slot)?;
        Ok((self.pair(primary, core)?, self.pair(secondary, core)?))
    }

    fn refresh_prepared_attacks(&self, c: &Character, attrs: &[i32; 7], s: &mut Scratch) -> R<()> {
        for (rec, slot) in [0, 1, 2, 3, 13].into_iter().enumerate() {
            let item = &c.inv[c.active * BANK + slot * ITEM..][..ITEM];
            let key = u16_at(item, 0);
            let Some(original) = (key != 0).then(|| self.items.row(u32::from(key))).flatten()
            else {
                continue;
            };
            let admitted = self.requirements(c.cap, attrs, original, item);
            let (first, second) =
                self.prepared_attack_pair(c, &s.transformed, item, original, slot, admitted)?;
            put_u32(&mut s.stat, 0x24 + 64 * rec, first);
            put_u32(&mut s.stat, 0x28 + 64 * rec, second);
        }
        Ok(())
    }

    fn prepared(&self, c: &Character, attrs: &[i32; 7], s: &mut Scratch) -> R<()> {
        let bank = c.active;
        for rec in 0..5 {
            let slot = [0, 1, 2, 3, 13][rec];
            let item = &c.inv[bank * BANK + slot * ITEM..][..ITEM];
            let key = u16_at(item, 0) as u32;
            let known = key != 0 && self.items.index(key).is_some();
            let at = 0x24 + 64 * rec;
            if rec == 4 && !known {
                continue;
            }
            if !known {
                let r = &mut s.stat[at..at + 64];
                for (off, v) in [
                    (0, c.unarmed),
                    (4, c.unarmed),
                    (0x18, u32::MAX),
                    (8, 0),
                    (0x34, 0),
                    (0xC, 2),
                    (0x10, 20),
                    (0x14, 10),
                    (0x38, 13),
                ] {
                    put_u32(r, off, v);
                }
                for off in [0x1C, 0x24, 0x2C] {
                    put_u64(r, off, 0);
                }
                r[0x3C..0x3E].copy_from_slice(&0x100u16.to_le_bytes());
                continue;
            }
            let original = self.items.row(key).ok_or("weapon outside the item table")?;
            let admitted = self.requirements(c.cap, attrs, original, item);
            let (first, second) =
                self.prepared_attack_pair(c, &s.transformed, item, original, slot, admitted)?;
            let aux0 = self.auxiliary(item, c.style, false)?;
            let aux1 = self.auxiliary(item, c.style, true)?;
            let fraction = |off: usize| {
                let v = u32_at(original, off);
                if v != 0 {
                    v as f32 * self.c_percent
                } else {
                    self.c_one
                }
            };
            let r = &mut s.stat[at..at + 64];
            put_u32(r, 0, first);
            put_u32(r, 8, 0);
            put_u32(r, 4, second);
            put_u32(r, 0xC, original[0x19B] as u32);
            put_u32(r, 0x10, aux0);
            put_u32(r, 0x18, u32::MAX);
            put_u32(r, 0x14, aux1);
            put_u32(r, 0x1C, 0);
            put_f32(r, 0x20, fraction(0xE4));
            put_u32(r, 0x24, u32_at(original, 0xE8));
            put_u32(r, 0x28, u32_at(original, 0xEC));
            put_f32(r, 0x2C, u32_at(original, 0xF0) as f32 * self.c_percent);
            put_f32(r, 0x30, fraction(0xF4));
            put_u32(r, 0x34, u32_at(original, 0x48));
            put_u32(r, 0x38, original[0x19A] as u32);
            r[0x3C] = ((u32_at(original, 0xB0) >> 3) & 1) as u8;
            r[0x3D] = 0;
            let held = (c.inv[bank * BANK + 0xF68] == 0) as usize;
            let mask = if slot == held || slot == 13 { 255 } else { 0 };
            if admitted {
                self.collect(&mut s.source, item, false, bank as i32, mask)?;
            }
        }
        for slot in [0, 1, 2, 3, 13] {
            let item = &c.inv[(bank ^ 1) * BANK + slot * ITEM..][..ITEM];
            let key = u16_at(item, 0) as u32;
            if key == 0 {
                continue;
            }
            let Some(p) = self.items.row(key) else {
                continue;
            };
            if self.requirements(c.cap, attrs, p, item) {
                self.collect(&mut s.source, item, true, (bank ^ 1) as i32, 255)?;
            }
        }
        Ok(())
    }

    fn defense(&self, c: &Character, item: &[u8], admitted: bool, e: Effective) -> R<u32> {
        let (_, p) = self.selection(item)?;
        if u32_at(item, 0x18) & 0x40000 != 0 {
            return Err("defense flag 40000 needs the category row");
        }
        let level = e.level.min(190);
        let lc = self.field70(level, 0x26);
        let pc = self.field70(e.plus.min(c.plus_cap), 0x28);
        let raw_plus = (u16_at(item, 0xA) as u32).min(c.cap);
        let (mut delta1, mut delta2) = (0u32, 0u32);
        let kind = u32_at(p, 0x60);
        if item[0x30] as i8 >= 3 {
            let a = ((item[0x31] == 1) as i32 + (item[0x32] == 1) as i32) as f32 * self.c_half;
            let b = ((item[0x31] == 2) as i32 + (item[0x32] == 2) as i32) as f32 * self.c_half;
            let (oa, ob) = match kind {
                0x2B2F => (0x68, 0x70),
                0x403B => (0x66, 0x6E),
                _ => (0x64, 0x6C),
            };
            delta1 = cvtt64(self.field70(raw_plus, oa) as f32 * a) as u32;
            delta2 = cvtt64(self.field70(raw_plus, ob) as f32 * b) as u32;
        }
        let product = (lc + pc).wrapping_mul(u32_at(p, 0xF8));
        let flat = if admitted {
            self.flat_group(item, 0xDA05)
        } else {
            0
        };
        Ok((product / 1000)
            .wrapping_add(u32_at(p, 0x50))
            .wrapping_add(delta1)
            .wrapping_add(delta2)
            .wrapping_add(flat))
    }

    fn weight(&self, c: &Character, item: &[u8], selected: &[u8]) -> u32 {
        let count = if item[0x30] as i8 >= 3 {
            (item[0x31] == 2) as i32 + (item[0x32] == 2) as i32
        } else {
            0
        };
        let factor = count as f32 * self.c_half;
        let plus = (u16_at(item, 0xA) as u32).min(c.cap);
        let delta = cvtt64(self.field70(plus, 0x6A) as f32 * factor) as u32;
        let total = u32_at(selected, 0x98).wrapping_add(delta);
        cvtt64(total as f32) as u32
    }

    fn body_field(
        &self,
        c: &Character,
        original: &[u8],
        q: u16,
        e: Effective,
        second: bool,
    ) -> R<i16> {
        if u32_at(original, 0x60) != 0x2D32 {
            return Ok(q as i16);
        }
        let level = e.level.min(190);
        let plus = e.plus.min(c.plus_cap);
        if level.max(plus) as usize >= self.base70.len() / 0x94 {
            return Err("missing table70 row for a body field");
        }
        let off = if second { 0x30 } else { 0x2C };
        let value = (self.field70(level, off) + self.field70(plus, off + 2)) as f32
            * self.c_4688
            * q as f32;
        Ok(cvtt32(value) as u16 as i16)
    }

    fn body(&self, c: &Character, attrs: &[i32; 7], s: &mut Scratch) -> R<()> {
        let stat = &mut s.stat;
        for &(off, _, _) in &BODY_FLOATS {
            finite(f32_at(stat, off))?;
        }
        let refresh = stat[0x7D0];
        for (reserve, bank) in [(false, c.active), (true, c.active ^ 1)] {
            for slot in 4..13 {
                let item = &c.inv[bank * BANK + slot * ITEM..][..ITEM];
                let key = u16_at(item, 0) as u32;
                if key == 0 {
                    continue;
                }
                let Some(original) = self.items.row(key) else {
                    continue;
                };
                let qualifies = self.requirements(c.cap, attrs, original, item);
                if !reserve {
                    let (_, selected) = self.selection(item)?;
                    if u32_at(item, 0x18) & 0x40000 != 0 {
                        return Err("toughness flag 40000 needs the category row");
                    }
                    let e = self.effective(c.cap, item, true)?;
                    let defense = self.defense(c, item, qualifies, e)?;
                    put_u32(stat, 0x1F4 + 4 * (slot - 4), defense);
                    let weight = self.weight(c, item, selected);
                    for (at, v) in [
                        (0x1C8, defense),
                        (0x218, u32_at(selected, 0xFC)),
                        (0x7B8, weight),
                    ] {
                        let n = u32_at(stat, at).wrapping_add(v);
                        put_u32(stat, at, n);
                    }
                    let v = u16_at(stat, 0x21C).wrapping_add(u16_at(original, 0x138));
                    stat[0x21C..0x21E].copy_from_slice(&v.to_le_bytes());
                    for &(off, parameter, second) in &BODY_FLOATS {
                        let term =
                            self.body_field(c, original, u16_at(original, parameter), e, second)?;
                        let v = term as f32 + f32_at(stat, off);
                        put_f32(stat, off, v);
                    }
                }
                if refresh == 0 && qualifies {
                    self.collect(
                        &mut s.source,
                        item,
                        reserve,
                        bank as i32,
                        if reserve { 0 } else { 255 },
                    )?;
                }
            }
        }
        Ok(())
    }

    fn slot15(&self, c: &Character, s: &mut Scratch) -> R<u32> {
        let mut attack_gain = 0;
        let item = &c.inv[c.active * BANK + 15 * ITEM..][..ITEM];
        let key = u16_at(item, 0) as u32;
        if let Some(parameter) = self.items.row(key).filter(|_| key != 0) {
            let e = self.effective(c.cap, item, false)?;
            let level = e.level.min(190);
            let plus = e.plus.min(c.plus_cap);
            if plus > 500 || level.max(plus) as usize >= self.base70.len() / 0x94 {
                return Err("slot 15 needs an unbound table70 row");
            }
            let product = (self.field70(level, 0x3E) + self.field70(plus, 0x40))
                .wrapping_mul((i32_at(parameter, 0xD4)).max(0) as u32)
                as i32;
            let attack = (u32_at(parameter, 0x10).wrapping_add((product / 1000) as u32) as i32)
                .max(1) as u32;
            let product = (self.field70(level, 0x42) + self.field70(plus, 0x44))
                .wrapping_mul(u32_at(parameter, 0xF8));
            let defense =
                (u32_at(parameter, 0x50).wrapping_add(product / 1000) as i32).max(1) as u32;
            attack_gain = attack & 65535;
            let v = u32_at(&s.stat, 0x1C8).wrapping_add(defense);
            put_u32(&mut s.stat, 0x1C8, v);
            self.collect(&mut s.source, item, false, c.mode, 255)?;
        }
        let reserve = &c.inv[(if c.mode == 1 { 0 } else { 1 }) * BANK + 15 * ITEM..][..ITEM];
        let key = u16_at(reserve, 0) as u32;
        if key != 0 && self.items.index(key).is_some() {
            for i in 0..7 {
                let effect = u32_at(reserve, 0x38 + 24 * i);
                if effect == u32::MAX {
                    continue;
                }
                let Some(row) = self.effects.row(effect) else {
                    continue;
                };
                let group = u16_at(row, 2);
                if matches!(group, 0xF79F | 0x62CB | 0x17B4)
                    && self.groups.index(group as u32).is_some()
                {
                    self.aggregate(&mut s.source, c.mode, group, u32_at(reserve, 0x3C + 24 * i));
                }
            }
        }
        Ok(attack_gain)
    }

    // Original slot 15 contributes once to both attack channels of all six records.
    // Apply after the final weapon preparation, including the selected-source refresh.
    fn add_slot15_attack(stat: &mut [u8], gain: u32) {
        for index in 0..6 {
            for offset in [0x24, 0x28] {
                let at = offset + 64 * index;
                put_u32(stat, at, u32_at(stat, at).wrapping_add(gain));
            }
        }
    }

    fn curve(&self, reward: &[u8], coordinate: u16) -> u32 {
        let count = u32_at(&self.level_rows, 4);
        let index = (coordinate as u32).min(500);
        if index >= count {
            return u32::MAX;
        }
        let mode = u32_at(reward, 12);
        let value = if mode <= 4 {
            u16_at(
                &self.level_rows,
                8 + index as usize * 10 + 2 * mode as usize,
            ) as i32
        } else {
            0
        };
        let product = value.wrapping_mul(i32_at(reward, 20));
        u32_at(reward, 16).wrapping_add((product / 1000) as u32)
    }

    fn set_rewards(&self, mode: i32, source: &mut Source) -> R<()> {
        for family in self.families {
            let bank = selected_map(family, source.mode);
            let reduction = source.maps[bank]
                .iter()
                .any(|e| e.key == SET_REDUCTION_GROUP) as u32;
            source.set_reduction[bank] = reduction != 0;
            source.set_pieces[bank] = source.sides[bank]
                .iter()
                .map(|entry| (u32_at(entry, 0), usize::from(u16_at(entry, 4))))
                .collect();
            for index in 0..source.sides[bank].len() {
                put_u32(&mut source.sides[bank][index], 8, reduction);
                let raw = source.sides[bank][index];
                if (u16_at(&raw, 4) as u32 + reduction) <= 1 {
                    continue;
                }
                let Some(parameter) = self.sides.row(u32_at(&raw, 0)) else {
                    continue;
                };
                for reward in 0..7 {
                    let row = &parameter[0x30 + 24 * reward..0x48 + 24 * reward];
                    let (key, threshold, flags) = (u16_at(row, 8), u32_at(row, 0), row[4]);
                    if key == 0 || (i32_at(&raw, 12) != mode && flags & 2 == 0) {
                        continue;
                    }
                    let count = u16_at(&raw, if flags & 1 != 0 { 6 } else { 4 }) as u32;
                    if count + reduction < threshold {
                        continue;
                    }
                    let coordinate = threshold.wrapping_sub(reduction) as i32;
                    let offset = if coordinate > 0 {
                        0x1C + 2 * coordinate as i64
                    } else {
                        0x1E
                    };
                    if offset < 0 || offset + 2 > 68 {
                        return Err("reward coordinate past the side entry");
                    }
                    let value = self.curve(row, u16_at(&raw, offset as usize));
                    #[cfg(test)]
                    source.set_row_activations.push(SetRowActivation {
                        side: u32_at(&raw, 0),
                        bank,
                        row_index: reward,
                        threshold,
                        reward_key: key,
                        flags,
                    });
                    self.aggregate(source, mode, key, value);
                }
            }
        }
        Ok(())
    }

    fn sample(&self, row: &[u8], grade: u32) -> R<f32> {
        let count = u16_at(row, 0x52) as u32;
        let effective = (if grade >= count {
            count.wrapping_sub(1)
        } else {
            grade
        })
        .min(17);
        finite(f32_at(row, 8 + 4 * effective as usize))
    }

    fn grade(&self, key: u16, x: i32, grade: u32) -> R<f32> {
        let (start, count) = self.grade_index[key as usize];
        if count == 0 {
            return Ok(0.0);
        }
        let rows = &self.grade_rows[start as usize * 84..(start + count) as usize * 84];
        let coordinate = |i: usize| i32_at(rows, i * 84 + 4);
        let row = |i: usize| &rows[i * 84..(i + 1) * 84];
        let (mut lo, mut hi) = (0usize, count as usize);
        while lo < hi {
            let mid = (lo + hi) / 2;
            if coordinate(mid) < x {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        if lo == 0 {
            return self.sample(row(0), grade);
        }
        if lo == count as usize {
            return self.sample(row(lo - 1), grade);
        }
        let (low, high) = (
            self.sample(row(lo - 1), grade)?,
            self.sample(row(lo), grade)?,
        );
        let distance = x.wrapping_sub(coordinate(lo - 1));
        let span = coordinate(lo).wrapping_sub(coordinate(lo - 1));
        if span <= 0 {
            return Err("grade interpolation outside its positive span");
        }
        let fraction = (distance as f32 / span as f32).clamp(0.0, 1.0);
        finite(fraction * (high - low) + low)
    }

    fn item_key(&self, inv: &[u8], bank: i32, slot: i32) -> u16 {
        let at = if (0..2).contains(&bank) && (0..17).contains(&slot) {
            bank as usize * BANK + slot as usize * ITEM
        } else {
            0
        };
        u16_at(inv, at)
    }

    /// Native item identity and type class from the unchanged1A0-byte item row.
    pub fn item_type_class(&self, key: u16) -> Option<u32> {
        self.items
            .row(u32::from(key))
            .filter(|row| u16_at(row, 0x152) == key)
            .map(|row| u32_at(row, 0x15C))
            .filter(|class| *class <= 64)
    }

    pub fn weapon_categories(&self, equipped: u16) -> Vec<u32> {
        let Some(class) = self.item_type_class(equipped) else {
            return Vec::new();
        };
        self.categories_for_weapon_class(class)
    }

    pub fn categories_for_weapon_class(&self, class: u32) -> Vec<u32> {
        let mut categories = self.items.rows[8..]
            .chunks_exact(self.items.stride)
            .filter(|row| u32_at(row, 0x15c) == class)
            .map(|row| u32_at(row, 0x58))
            .collect::<Vec<_>>();
        categories.sort_unstable();
        categories.dedup();
        categories
    }

    pub fn category(&self, key: u16) -> Option<u32> {
        self.items.row(key as u32).map(|r| u32_at(r, 0x58))
    }

    fn materialize_one(
        &self,
        c: &Character,
        attrs: &[i32; 7],
        group: u16,
        v: u32,
        stat: &mut [u8],
        d: &mut [u8],
    ) -> R<()> {
        let (class, start, n) = self.class[group as usize];
        let add_u32 = |stat: &mut [u8], at: usize, by: u32| {
            let x = u32_at(stat, at).wrapping_add(by);
            put_u32(stat, at, x);
        };
        fn add_f32(b: &mut [u8], at: usize, by: f32) -> R<()> {
            let old = finite(f32_at(b, at))?;
            put_f32(b, at, finite(by + old)?);
            Ok(())
        }
        let bank = i32_at(&c.inv, 0x1EE0);
        let selected_slot = || -> R<i32> {
            if !(0..2).contains(&bank) {
                return Err("selector byte outside the inventory");
            }
            Ok((c.inv[bank as usize * BANK + 0xF68] == 0) as i32)
        };
        match class {
            NO_WRITE => {}
            RECIPES => {
                for r in &self.recipes[start as usize..(start + n) as usize] {
                    let at = r.offset;
                    match r.kind {
                        LITERAL => {
                            d[at..at + r.width].copy_from_slice(&r.literal.to_le_bytes()[..r.width])
                        }
                        D_SET | D_ADD => {
                            let mut value = v as f32;
                            if let Some(scale) = r.scale {
                                value *= scale;
                            }
                            if r.kind == D_ADD {
                                value += finite(f32_at(d, at))?;
                            }
                            put_f32(d, at, finite(value)?);
                        }
                        D_OLD_MAX => {
                            let old = u32_at(d, at);
                            put_u32(d, at, old.wrapping_add((old as i32).max(v as i32) as u32));
                        }
                        D_MAX => {
                            let old = u32_at(d, at);
                            put_u32(d, at, (old as i32).max(v as i32) as u32);
                        }
                        D_OLD_V => add_u32(d, at, v),
                        STAT_ADD_F => {
                            let mut term = v as f32;
                            if let Some(scale) = r.scale {
                                term *= scale;
                            }
                            add_f32(stat, at, term)?;
                        }
                        STAT_ADD_U16 => {
                            let x = u16_at(stat, at).wrapping_add(v as u16);
                            stat[at..at + 2].copy_from_slice(&x.to_le_bytes());
                        }
                        STAT_ADD_U32 => add_u32(stat, at, v),
                        _ => return Err("unknown recipe kind"),
                    }
                }
            }
            STATEFUL => match group {
                0x8C47 => {
                    for (flag, at) in [(0x61, 0x2C), (0xA1, 0x6C)] {
                        if stat[flag] == 0 {
                            add_u32(stat, at, v);
                        }
                    }
                }
                0x8703 => {
                    if stat[0x161] == 0 && u32_at(stat, 0x124) != 0 {
                        add_u32(stat, 0x12C, v);
                    }
                }
                g if ACTIVE_CATEGORY.contains(&g) => {
                    let slot = selected_slot()?;
                    let target = u32_at(self.groups.row(g as u32).ok_or("missing group")?, 0x20);
                    let key = self.item_key(&c.inv, bank, slot);
                    let category = if key != 0 {
                        self.category(key).unwrap_or(0x3C2E)
                    } else {
                        0x3C2E
                    };
                    if category == target {
                        for (curve, at) in [(0x1DF9, 0xCC), (0x874A, 0xD0)] {
                            add_f32(d, at, self.grade(curve, 0, v)? * self.c_percent)?;
                        }
                    }
                }
                0xDCBD | 0xB00A | 0x39B8 | 0x24EE | 0xE5B9 | 0x09B1 | 0x8997 => {
                    let index = match group {
                        0xDCBD => 0,
                        0xB00A => 1,
                        0x39B8 => 2,
                        0x24EE => 3,
                        0xE5B9 => 4,
                        0x09B1 => 5,
                        _ => 6,
                    };
                    add_u32(
                        stat,
                        0x1C8,
                        cvtt64(self.grade(0x6498, attrs[index], v)?) as u32,
                    );
                }
                0x3503 | 0x684F | 0xBB6D | 0x794D | 0xEEEF | 0xA71E => {
                    let tag = match group {
                        0x3503 => 0,
                        0x684F => 1,
                        0xBB6D => 2,
                        0x794D => 22,
                        0xEEEF => 3,
                        _ => 4,
                    };
                    let at = 0x3C + 64 * selected_slot()? as usize;
                    if u32_at(stat, at) == u32::MAX {
                        put_u32(stat, at, tag);
                        put_u32(stat, at + 4, v);
                    }
                }
                0x3000 | 0xB2FF => {
                    let empty = self
                        .slots8
                        .iter()
                        .filter(|&&s| self.item_key(&c.inv, bank, s) == 0)
                        .count() as i32;
                    let (curve, at) = if group == 0x3000 {
                        (0x8AF7, 0x108)
                    } else {
                        (0xA9A, 0x294)
                    };
                    add_f32(d, at, self.grade(curve, empty, v)? * self.c_percent)?;
                }
                0x2C9E | 0x2E5B => {
                    let (curve, index, at) = if group == 0x2C9E {
                        (0xBE72, 6, 0x684)
                    } else {
                        (0xF919, 4, 0x794)
                    };
                    add_f32(d, at, self.grade(curve, attrs[index], v)? * self.c_percent)?;
                }
                0x5D3A => {
                    let factor = self.grade(0xFA3E, 1, v)? * self.c_percent;
                    add_u32(
                        stat,
                        0x18,
                        cvtt64(factor * self.field90(attrs[2], 0xC) as f32) as u32,
                    );
                }
                0xD84B => {
                    let x = cvtt64(finite(f32_at(stat, 0x55C))?) as i32;
                    let value = self.grade(0xA7C5, x, v)? * self.c_percent - self.c_one;
                    add_f32(d, 0xCC, value)?;
                }
                0xDFC6 => {
                    let value = finite(finite(f32_at(stat, 0x7B0))? - v as f32 * self.c_4688)?;
                    put_f32(
                        stat,
                        0x7B0,
                        if self.opt_d1f <= value {
                            value
                        } else {
                            self.opt_d1f
                        },
                    );
                }
                0xB3FE => {
                    let old = u32_at(stat, 0x7B4);
                    let term = old as f32 * v as f32 * self.c_4688;
                    put_u32(stat, 0x7B4, (old as i64).wrapping_add(cvtt64(term)) as u32);
                }
                0xB148 => {
                    let denominator = self.opt_22af * self.opt_1f32 * self.c_4080;
                    let accumulated =
                        finite(v as f32 / denominator + finite(f32_at(stat, 0x7A4))?)?;
                    put_f32(stat, 0x7A4, accumulated);
                    put_f32(
                        stat,
                        0x7A8,
                        finite(self.opt_1f32 * self.opt_22af * accumulated)?,
                    );
                }
                0xCBEE | 0xE63F => {
                    let count = self
                        .slots5
                        .iter()
                        .filter(|&&s| {
                            self.items
                                .row(self.item_key(&c.inv, bank, s) as u32)
                                .is_some_and(|r| u32_at(r, 0xB0) & 0x100 != 0)
                        })
                        .count();
                    if group == 0xCBEE {
                        add_f32(d, 0xCC, v as f32 * self.c_4688 * count as f32)?;
                    } else if count >= 5 {
                        add_u32(d, 0x61C, 1);
                    }
                }
                _ => return Err("unclassified stateful group"),
            },
            CONDITIONAL => match group {
                0xF919 | 0x052D => {
                    if c.mode == (group == 0x052D) as i32 {
                        add_u32(stat, 0x2C, v);
                        add_u32(stat, 0x6C, v);
                    }
                }
                _ => {
                    let fixed = FIXED_CATEGORY.iter().find(|f| f.0 == group);
                    let target = match fixed {
                        Some(f) => f.1,
                        None => u32_at(self.groups.row(group as u32).ok_or("missing group")?, 0x20),
                    };
                    for slot in 0..2 {
                        let key = self.item_key(&c.inv, bank, slot);
                        let mut category = self.category(key);
                        if fixed.is_none() && (key == 0 || category.is_none()) {
                            category = Some(0x3C2E);
                        }
                        if category == Some(target) {
                            add_u32(stat, 0x2C + 0x40 * slot as usize, v);
                        }
                    }
                }
            },
            _ => return Err("group outside the materializer table"),
        }
        Ok(())
    }

    pub(crate) fn set_reduction_before_equipment(&self, f: &Frame) -> bool {
        let in_prefix = match &f.source_rebuild {
            Some(program) => program
                .prefix
                .iter()
                .any(|op| op.group == SET_REDUCTION_GROUP),
            None => f
                .native_before
                .iter()
                .any(|&(group, _)| group == SET_REDUCTION_GROUP),
        };
        // Set rewards write to the selected mode, which may be visited after
        // the other family. Include any possible earlier reduction reward.
        in_prefix
            || self.sides.rows[8..]
                .chunks_exact(self.sides.stride)
                .any(|side| {
                    (0..7).any(|reward| u16_at(side, 0x38 + 24 * reward) == SET_REDUCTION_GROUP)
                })
    }

    pub(crate) fn add_set_capacity(
        &self,
        capacity: &mut SetCapacity,
        keys: &[u32],
        c: &Character,
        attrs: &[i32; 7],
        item: &[u8],
        requires_attributes: bool,
    ) {
        let key = u16_at(item, 0);
        let Some(original) = self.items.row(u32::from(key)).filter(|_| key != 0) else {
            return;
        };
        if requires_attributes && !self.requirements(c.cap, attrs, original, item) {
            return;
        }
        for (_, group, side, _) in self.effect_lines(item) {
            if side == 0 {
                // Inclusion is independent of amount. Masks and collector
                // width may suppress a submission but cannot add one.
                capacity.reduction |= group == SET_REDUCTION_GROUP;
            } else if let Some(index) = keys.iter().position(|&key| key == side) {
                capacity.pieces[index] += 1;
            }
        }
    }

    /// What dominance compares about one item: summed effect values of the groups that matter, set memberships, weight,
    /// level, the 0x100 item flag and defense. Effects of an item whose attribute requirements the start points miss are
    /// left out, as the game drops them.
    /// Each effect line of an item: (effect id, group, set side key or 0, value).
    pub fn effect_lines(&self, item: &[u8]) -> Vec<(u32, u16, u32, u32)> {
        let mut out = Vec::new();
        for i in 0..7 {
            let effect = u32_at(item, 0x38 + 24 * i);
            let Some(row) = (effect != u32::MAX)
                .then(|| self.effects.row(effect))
                .flatten()
            else {
                continue;
            };
            let group = u16_at(row, 2);
            let side = self.groups.row(group as u32).map_or(0, |g| u32_at(g, 8));
            out.push((effect, group, side, u32_at(item, 0x3C + 24 * i)));
        }
        out
    }

    pub fn profile(
        &self,
        c: &Character,
        attrs: &[i32; 7],
        item: &[u8],
        live: &std::collections::HashSet<u16>,
    ) -> Option<Profile> {
        let (original, selected) = self.selection(item).ok()?;
        let qualified = self.requirements(c.cap, attrs, original, item);
        let weight = self.weight(c, item, selected) as i64;
        let mut values: Vec<(u16, u64)> = Vec::new();
        let mut sides: Vec<u32> = Vec::new();
        if qualified {
            for i in 0..7 {
                let effect = u32_at(item, 0x38 + 24 * i);
                let Some(row) = (effect != u32::MAX)
                    .then(|| self.effects.row(effect))
                    .flatten()
                else {
                    continue;
                };
                let group = u16_at(row, 2);
                let Some(g) = self.groups.row(group as u32) else {
                    continue;
                };
                let side = u32_at(g, 8);
                if side != 0 {
                    if !sides.contains(&side) {
                        sides.push(side);
                    }
                } else if live.contains(&group) {
                    let v = u32_at(item, 0x3C + 24 * i) as u64;
                    match values.iter_mut().find(|e| e.0 == group) {
                        Some(e) => e.1 += v,
                        None => values.push((group, v)),
                    }
                }
            }
        }
        sides.sort_unstable();
        values.sort_unstable();
        let level = if u32_at(item, 0x18) & 0x200000 != 0 {
            0
        } else {
            (u16_at(item, 6) as u32).min(180)
        };
        // Use the evaluator's exact item contribution, including enhancement,
        // familiarity, rarity, remodels, caps and admitted flat effects.
        // Keep known effects and set memberships even if defense is unknown:
        // those fields also classify the search space before dominance.
        let defense = self
            .effective(c.cap, item, true)
            .and_then(|effective| self.defense(c, item, qualified, effective))
            .ok()
            .map(u64::from);
        Some(Profile {
            values,
            sides,
            weight,
            level,
            flag: u32_at(original, 0xB0) & 0x100 != 0,
            defense,
        })
    }

    /// Carrying capacity of both stances at these points; the smaller one bounds the load.
    pub fn capacities(&self, attrs: &[i32; 7]) -> (u32, u32) {
        let at = attrs[2].clamp(0, 999) as usize * 32 + 8;
        (
            u16_at(&self.power90, at) as u32,
            u16_at(&self.power90, at + 2) as u32,
        )
    }

    /// Equipment load as the game shows it: carried weight over capacity.
    pub fn load_of(&self, _c: &Character, _attrs: &[i32; 7], stat: &[u8]) -> f32 {
        u32_at(stat, 0x7B8) as f32 / u32_at(stat, 0x7B4) as f32
    }

    /// `_pipeline`: leaves the materialized STAT, D and final aggregate in the scratch.
    pub fn pipeline(
        &self,
        f: &Frame,
        c: &Character,
        attrs: &[i32; 7],
        ops: &[(u16, i64)],
        s: &mut Scratch,
    ) -> R<()> {
        s.source_rebuild_trace = None;
        s.legacy_source_signature = None;
        if let Some(program) = &f.source_rebuild {
            return program.pipeline(self, f, c, attrs, ops, s);
        }
        let gain = self.collect_pipeline(f, c, attrs, ops, s, None)?;
        Self::add_slot15_attack(&mut s.stat, gain);
        self.materialize_pipeline(c, attrs, s)
    }

    fn collect_pipeline(
        &self,
        f: &Frame,
        c: &Character,
        attrs: &[i32; 7],
        ops: &[(u16, i64)],
        s: &mut Scratch,
        program: Option<(&source_rebuild::Program, &[source_rebuild::Submission])>,
    ) -> R<u32> {
        #[cfg(test)]
        s.source.set_row_activations.clear();
        s.stat.copy_from_slice(&f.stat);
        // Original818E85..818ED4: Heart row90 u16+4, binary32 scaling,
        // CVTTSS2SI64, then equipment/qualification/source materialization.
        // The declared zero descriptor makes original376698 return exactly1.
        // Never fit this factor from the captured maximum or subtract gear from it.
        if f.identity_ki_seed {
            put_u32(&mut s.stat, 0x1C, self.field90(attrs[1], 4));
        } else if f.held_reaper && attrs[1] != i32_at(&f.stat, 0x848) {
            return Err("changed Heart requires the explicit Ki seed contract");
        }
        for (offset, index, column) in [(0x1BC, 4, 0x10), (0x1C0, 4, 0x12), (0x1C4, 6, 0x18)] {
            put_u32(&mut s.stat, offset, self.field90(attrs[index], column));
        }
        put_u32(
            &mut s.stat,
            0x7B4,
            self.field90(attrs[2], if c.mode == 0 { 8 } else { 10 }),
        );
        let recovery = self.field90(attrs[5], 0x16) as f32 * self.c_4894
            + self.field90(attrs[1], 6) as f32 * self.c_4894;
        put_f32(&mut s.stat, 0x7A4, recovery);
        put_f32(&mut s.stat, 0x7A8, self.opt_recovery * recovery);
        put_f32(
            &mut s.stat,
            0x1A8,
            self.field90(attrs[3], 0xE) as f32 * self.c_4894 + self.c_one,
        );
        put_f32(
            &mut s.stat,
            0x1AC,
            self.field90(attrs[5], 0x14) as f32 * self.c_4894 + self.c_one,
        );
        put_f32(&mut s.stat, 0x7B0, self.opt_e40f);
        // A removed item also clears its formerly occupied defense field.
        s.stat[0x1F4..0x218].fill(0);
        // Weight is rebuilt from the equipped slots, not added to captured worn weight.
        put_u32(&mut s.stat, 0x7B8, 0);
        s.d.copy_from_slice(&f.d0);
        for i in 0..5 {
            put_u64(&mut s.stat, 0x24 + 64 * i, 0);
            put_u32(&mut s.stat, 0x2C + 64 * i, 0);
        }
        put_u64(&mut s.stat, 8, 0);
        put_u64(&mut s.stat, 0x7D8, f.attr);
        s.stat[0x7D0] = 0;
        s.source.mode = c.mode;
        for (i, &points) in attrs.iter().enumerate() {
            s.source.thresholds[i] = points as u32;
            let index = points.clamp(1, 999) as u32;
            if index >= u32_at(&self.base90, 4) {
                return Err("table90 reader would return null");
            }
            s.transformed[i] = u16_at(&self.base90, 8 + 32 * index as usize + 0x1A) as i32;
        }
        for b in 0..2 {
            s.source.maps[b].clear();
            s.source.sides[b].clear();
            s.source.set_reduction[b] = false;
            s.source.set_pieces[b].clear();
        }
        if let Some((_, prefix)) = program {
            for entry in prefix {
                self.aggregate(&mut s.source, c.mode, entry.group, entry.value);
            }
        } else {
            for &(group, value) in &f.native_before {
                self.aggregate(&mut s.source, c.mode, group, value);
            }
        }
        self.prepared(c, attrs, s)?;
        self.body(c, attrs, s)?;
        // Original817515 skips only the active contribution. Reserve slot15
        // remains eligible when the active slot is empty (81751E onward).
        let slot15_attack = self.slot15(c, s)?;
        self.set_rewards(c.mode, &mut s.source)?;
        if program.is_none() {
            s.legacy_source_signature = Some(self.legacy_source_signature(c, attrs, &s.source)?);
        }
        if let Some((program, _)) = program {
            program.after_sets(self, c, s)?;
        } else {
            for &(group, value) in &f.native_after {
                self.aggregate(&mut s.source, c.mode, group, value);
            }
        }
        let bank = selected_map(c.mode, c.mode);
        let entries = &mut s.source.maps[bank];
        apply_operations(entries, ops)?;
        let capacity = self.field90(attrs[2], if c.mode == 0 { 8 } else { 10 });
        let ratio = u32_at(&s.stat, 0x7B8) as f32 / capacity as f32;
        put_f32(&mut s.stat, 0x7A0, self.opt_d84f + ratio * self.opt_dc22);
        put_u64(&mut s.stat, 8, f.owner);
        put_u64(&mut s.stat, 0x7C0, f.dest);
        Ok(slot15_attack)
    }

    fn materialize_pipeline(&self, c: &Character, attrs: &[i32; 7], s: &mut Scratch) -> R<()> {
        let bank = selected_map(c.mode, c.mode);
        self.materialize_entries(c, attrs, &s.source.maps[bank], &mut s.stat, &mut s.d)
    }

    fn materialize_entries(
        &self,
        c: &Character,
        attrs: &[i32; 7],
        entries: &[Entry],
        stat: &mut [u8],
        d: &mut [u8],
    ) -> R<()> {
        for entry in entries {
            self.materialize_one(
                c,
                attrs,
                entry.key,
                entry.base.wrapping_add(u32::from(entry.dup)),
                stat,
                d,
            )?;
        }
        Ok(())
    }

    /// `_evaluate` up to the formula: the words to write on the baseline frame.
    pub fn frame_writes(
        &self,
        f: &Frame,
        c: &Character,
        attrs: &[i32; 7],
        gain: u32,
        defense: u32,
        s: &mut Scratch,
    ) -> R<()> {
        if f.source_rebuild.is_some() && !f.source_baseline_ready {
            return Err(
                "source rebuild needs a matching current predictor baseline before scoring",
            );
        }
        let pred = &mut s.pred;
        pred.copy_from_slice(&f.stat);
        for at in (0..0x7A0).step_by(4) {
            if at == 8 || at == 12 {
                continue;
            }
            if matches!(at, 0x1A8 | 0x1AC | 0x1BC | 0x1C0 | 0x1C4)
                || (at == 0x1C && f.identity_ki_seed)
            {
                put_u32(pred, at, u32_at(&s.stat, at));
                continue;
            }
            let value = crate::stat_words::merge(
                at,
                u32_at(&f.s_base, at),
                u32_at(&s.stat, at),
                u32_at(pred, at),
            )?;
            put_u32(pred, at, value);
        }
        put_u32(pred, 0x7A0, u32_at(&s.stat, 0x7A0));
        put_u32(pred, 0x7B8, u32_at(&s.stat, 0x7B8));
        for i in 0..5 {
            let v = u32_at(&s.stat, 0x24 + 64 * i) as u64 + gain as u64;
            let secondary = u32_at(&s.stat, 0x28 + 64 * i) as u64 + gain as u64;
            if v > u32::MAX as u64 || secondary > u32::MAX as u64 {
                return Err("record field past 32 bits");
            }
            put_u32(pred, 0x24 + 64 * i, v as u32);
            put_u32(pred, 0x28 + 64 * i, secondary as u32);
        }
        // Every +8 source contribution is already materialized and copied by
        // the typed record merge; reconstructing a short group list loses writers.
        let f90 = |i: i32, off: usize| self.field90(i, off);
        put_u32(pred, 0x7B4, u32_at(&s.stat, 0x7B4));
        // Native 819254..8192B0 rebuilds agility after materializing equipment.
        // A captured score is not a candidate invariant when weight changes.
        let weight = u32_at(&s.stat, 0x7B8);
        let capacity = u32_at(pred, 0x7B4);
        let improvement = i32_at(&s.d, 0x61C);
        for (offset, unlocked) in [(0x798, false), (0x79C, s.d[0x6D1] != 0)] {
            let score = crate::agility::AgilityThresholds::PC202.score(
                weight,
                capacity,
                improvement,
                unlocked,
            );
            // Original819254..8192B0 can only raise the value to 20.
            // The actor predicate cannot change a score already at 20 or above.
            let score = if score < 20 && f.actor_minimum_agility()? {
                20
            } else {
                score
            };
            put_u32(pred, offset, score);
        }
        for at in [0x7A4, 0x7A8, 0x7B0] {
            put_u32(pred, at, u32_at(&s.stat, at));
        }
        // Original8192F9..819327 applies the positive D+A8 contribution after
        // the materializer, so reseeding later would erase it.
        let bonus = finite(f32_at(&s.d, 0xA8))?;
        if bonus > 0.0 {
            let value = finite(bonus + finite(f32_at(pred, 0x1AC))?)?;
            put_f32(pred, 0x1AC, value);
        }
        // Original819332..819367: multiply first, truncate64, add32.
        let scale = finite(f32_at(&s.d, 0x64))?;
        if scale >= 0.0 {
            let before = u32_at(pred, 0x1C4);
            let term = finite(before as f32 * scale)?;
            put_u32(pred, 0x1C4, before.wrapping_add(cvtt64(term) as u32));
        }
        let grade = i32_at(&s.d, 0x608);
        if grade >= 0 {
            let (current, maximum) = f.owner_resource.ok_or(
                "STAT capacity resource-curve7A62 requires an explicit owner-resource binding",
            )?;
            let ratio = if maximum <= 0 {
                0.0
            } else {
                finite(f32::from_bits(current) / maximum as f32)?.clamp(0.0, 1.0)
            };
            // Original81938D..8193C3: binary32 ratio*1000, CVTT64 then
            // signed32 coordinate, native grade interpolation, CVTT64/add32.
            let coordinate = cvtt64(finite(ratio * 1000.0)?) as i32;
            let increment = cvtt64(self.grade(0x7A62, coordinate, grade as u32)?) as u32;
            let before = u32_at(pred, 0x1C4);
            put_u32(pred, 0x1C4, before.wrapping_add(increment));
        }
        let hp = (f90(c.level as i32, 0) + f90(attrs[0], 2) + f90(attrs[2], 0xC)) as i64;
        let health = u32_at(pred, 0x18) as i64 + hp - f.hp_base as i64;
        let armor = u32_at(pred, 0x1C8) as i64 - f.baseline_defense_sources as i64 + defense as i64;
        if !(0..1 << 32).contains(&health) || !(0..1 << 32).contains(&armor) {
            return Err("health or defense word out of range");
        }
        put_u32(pred, 0x18, health as u32);
        put_u32(pred, 0x1C8, armor as u32);
        let writes = &mut s.writes;
        writes.clear();
        for at in (0..0x7C0).step_by(4) {
            if at != 8 && at != 12 && u32_at(pred, at) != u32_at(&f.stat, at) {
                writes.push((f.stat_addr + at as u64, u32_at(pred, at)));
            }
        }
        for at in (0..0xB00).step_by(4) {
            let (now, zero, base) = (u32_at(&s.d, at), u32_at(&f.d0, at), u32_at(&f.d_base, at));
            if now != zero || base != zero {
                writes.push((f.d_addr + at as u64, now));
            }
        }
        let packet = f
            .main8
            .wrapping_add(u32_at(pred, 0x24).wrapping_add(u32_at(pred, 0x2C)))
            .wrapping_sub(u32_at(&f.stat, 0x24).wrapping_add(u32_at(&f.stat, 0x2C)));
        writes.push((f.main_addr + 8, packet));
        let packet_kind = f.packet_kind.resolve(u32_at(pred, 0x30));
        writes.push((f.main_addr + 0xC, packet_kind));
        writes.push((f.main_addr + 0x10, u32_at(pred, 0x34)));
        let element = u32_at(pred, 0x3C);
        writes.push((f.main_addr + 0x14, element));
        if f.weapon_element_packet {
            // The constructor initializes power before the optional weapon
            // imbue. Restore it when the candidate removes the equipped imbue.
            let power = if element == u32::MAX {
                100
            } else {
                u32_at(pred, 0x40)
            };
            writes.push((f.main_addr + 0x18, power));
        }
        writes.push((f.ext_addr + 0x7B8, u32_at(pred, 0x7B8)));
        for (i, &points) in attrs.iter().enumerate() {
            writes.push((f.stat_addr + 0x844 + 4 * i as u64, points as u32));
        }
        Ok(())
    }
}

#[inline]
fn selected_map(context: i32, mode: i32) -> usize {
    ((if context == -1 { mode } else { context }) == 1) as usize
}

impl Frame {
    pub fn private_stat_digest(frame: &[u8]) -> R<String> {
        use sha2::{Digest, Sha256};
        Ok(format!(
            "{:x}",
            Sha256::digest(Sections::parse(frame)?.get("stat")?)
        ))
    }
    /// The public layout contains only canonical addresses and frame shape.
    /// Rebind this role's four buffers and numerical seeds in one private frame.
    pub fn first_use_frame(
        layout: &[u8],
        stat: &[u8],
        reset_stat: &[u8],
        damage: &[u8],
        reset_damage: &[u8],
        seeds: [u32; 3],
        resource: [u32; 2],
    ) -> R<Vec<u8>> {
        let s = Sections::parse(layout)?;
        let mut addresses = s.get("addresses")?.clone();
        if addresses.len() != 72
            || stat.len() != 0x900
            || reset_stat.len() != 0x900
            || damage.len() != 0xb00
            || reset_damage.len() != 0xb00
        {
            return Err("live direct-contact frame shape differs");
        }
        // Rebase the three typed role links in each STAT buffer. Other bytes
        // remain owned by this read, including preserved native padding.
        let mut current = stat.to_vec();
        let mut reset = reset_stat.to_vec();
        for buf in [&mut current, &mut reset] {
            for (at, index) in [(8usize, 4usize), (0x7c0, 6), (0x7d8, 5)] {
                buf[at..at + 8].copy_from_slice(&addresses[index * 8..index * 8 + 8]);
            }
        }
        addresses[56..64].copy_from_slice(&u64::from(seeds[0]).to_le_bytes());
        addresses[64..72].copy_from_slice(&u64::from(seeds[1]).to_le_bytes());
        let mut out = Vec::new();
        let mut section = |name: &str, data: &[u8]| {
            out.extend((name.len() as u32).to_le_bytes());
            out.extend(name.as_bytes());
            out.extend((data.len() as u32).to_le_bytes());
            out.extend(data);
        };
        section("addresses", &addresses);
        section("stat", &current);
        section("s_base", &reset);
        section("d0", reset_damage);
        section("d_base", damage);
        section("baseline_sources", &seeds[2].to_le_bytes());
        // These are attack policies compared by same_layout. In particular,
        // elemental projectiles and the resource-driven loop must survive
        // replacing the four actor buffers with this role's observations.
        for name in [
            "packet_kind",
            "aad_packet_kind",
            "weapon_element_packet",
            "hit_packets",
            "held_reaper",
            "identity_ki_seed",
        ] {
            if let Some(policy) = s.0.get(name) {
                section(name, &policy.bytes);
            }
        }
        Self::with_owner_resource(&out, resource)
    }
    /// Rebuild the predictor reference at the immutable context's attributes.
    /// The observed STAT stays untouched. This prevents adding a newly owned
    /// source program on top of an equipment-only comparison baseline.
    pub fn rebase_source(
        &mut self,
        tables: &Tables,
        character: &Character,
        attrs: &[i32; 7],
    ) -> R<()> {
        if self.source_rebuild.is_none() {
            return Ok(());
        }
        let baseline = self.source_rebuild.as_ref().and_then(|program| program.baseline.as_ref())
            .ok_or("source program requires an independently bound current STAT baseline before scoring")?;
        use sha2::{Digest, Sha256};
        if baseline.attrs != *attrs
            || baseline.stat_sha256 != format!("{:x}", Sha256::digest(&self.stat))
            || baseline.reset_d_sha256 != format!("{:x}", Sha256::digest(&self.d0))
            || (
                baseline.health_seed,
                baseline.defense_sources,
                baseline.main_base,
            ) != (self.hp_base, self.baseline_defense_sources, self.main8)
            || attrs
                .iter()
                .enumerate()
                .any(|(index, value)| i32_at(&self.stat, 0x844 + 4 * index) != *value)
        {
            return Err(
                "source program baseline is not the same actor/attributes as the current context",
            );
        }
        let mut scratch = Scratch::new();
        tables.pipeline(self, character, attrs, &[], &mut scratch)?;
        self.s_base = scratch.stat;
        // D is an absolute replacement. Preserve old owned clear sites as well
        // as the new program's writes so removed source effects are cleared.
        for at in (0..0xb00).step_by(4) {
            if u32_at(&scratch.d, at) != u32_at(&self.d0, at) {
                self.d_base[at..at + 4].copy_from_slice(&scratch.d[at..at + 4]);
            }
        }
        self.source_baseline_ready = true;
        Ok(())
    }

    pub fn has_source_rebuild(&self) -> bool {
        self.source_rebuild.is_some()
    }

    /// Identity of the immutable source producer bound to this frame.
    pub fn source_generation(&self) -> Option<&str> {
        self.source_rebuild
            .as_ref()
            .map(|program| program.binding.generation.as_str())
    }

    pub(crate) fn product_kernel_features(data: &[u8]) -> R<Vec<&'static str>> {
        let sections = Sections::parse(data)?;
        Ok([
            "owner_resource",
            "native_sources",
            "actor_agility",
            "source_rebuild",
        ]
        .into_iter()
        .filter(|name| sections.0.contains_key(*name))
        .collect())
    }

    /// Replace the complete source program atomically; no merge with an old
    /// generation is permitted. Producer inputs remain visible in the frame.
    pub fn with_source_rebuild(frame: &[u8], program: &source_rebuild::Program) -> R<Vec<u8>> {
        program.validate()?;
        let sections = Sections::parse(frame)?;
        let mut output = Vec::with_capacity(frame.len() + 8192);
        let mut at = 0;
        while at < frame.len() {
            let n = u32_at(frame, at) as usize;
            let size_at = at + 4 + n;
            let end = size_at + 4 + u32_at(frame, size_at) as usize;
            if &frame[at + 4..size_at] != b"source_rebuild" {
                output.extend_from_slice(&frame[at..end]);
            }
            at = end;
        }
        drop(sections);
        let raw = serde_json::to_vec(program).map_err(|_| "source rebuild serialization failed")?;
        let name = b"source_rebuild";
        output.extend_from_slice(&(name.len() as u32).to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(&(raw.len() as u32).to_le_bytes());
        output.extend_from_slice(&raw);
        Ok(output)
    }

    fn actor_minimum_agility(&self) -> R<bool> {
        let Some([tag, active, cancelled, known]) = self.actor_agility else {
            return Ok(false); // Original recorded-scenario compatibility.
        };
        if known & 1 == 0 {
            return Err("source actor tag is unknown for agility");
        }
        if tag & 15 != 0 {
            return Ok(false);
        }
        if known & 2 == 0 {
            return Err("source actor886 is unknown for agility");
        }
        if active == 0 {
            return Ok(false);
        }
        if known & 4 == 0 {
            return Err("source actor88A is unknown for agility");
        }
        Ok(cancelled == 0)
    }

    pub(crate) fn with_actor_agility(frame: &[u8], words: [u8; 4]) -> R<Vec<u8>> {
        let sections = Sections::parse(frame)?;
        if words[3] > 7 {
            return Err("invalid actor agility known mask");
        }
        if let Some(existing) = sections.0.get("actor_agility") {
            if existing.bytes != words {
                return Err("actor agility contradicts captured scene");
            }
            return Ok(frame.to_vec());
        }
        let mut output = frame.to_vec();
        let name = b"actor_agility";
        output.extend_from_slice(&(name.len() as u32).to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(&4u32.to_le_bytes());
        output.extend_from_slice(&words);
        Ok(output)
    }
    pub(crate) fn with_native_sources(
        frame: &[u8],
        before: &[(u16, u32)],
        after: &[(u16, u32)],
    ) -> R<Vec<u8>> {
        let _ = Sections::parse(frame)?;
        if before.len() + after.len() > 4096
            || before.iter().chain(after).any(|(group, _)| *group == 0)
        {
            return Err("native source submission extent or group is invalid");
        }
        // Replace an earlier generated program when refreshing a context. The
        // manifest remains the authoritative owner of source-layer semantics.
        let mut output = Vec::with_capacity(frame.len() + 32 + 8 * (before.len() + after.len()));
        let mut at = 0;
        while at < frame.len() {
            let n = u32_at(frame, at) as usize;
            let size_at = at + 4 + n;
            let end = size_at + 4 + u32_at(frame, size_at) as usize;
            if &frame[at + 4..size_at] != b"native_sources" {
                output.extend_from_slice(&frame[at..end]);
            }
            at = end;
        }
        if !before.is_empty() || !after.is_empty() {
            let name = b"native_sources";
            output.extend_from_slice(&(name.len() as u32).to_le_bytes());
            output.extend_from_slice(name);
            output.extend_from_slice(&(12 + 8 * (before.len() + after.len()) as u32).to_le_bytes());
            for word in [1, before.len() as u32, after.len() as u32] {
                output.extend_from_slice(&word.to_le_bytes());
            }
            for &(group, value) in before.iter().chain(after) {
                output.extend_from_slice(&(group as u32).to_le_bytes());
                output.extend_from_slice(&value.to_le_bytes());
            }
        }
        Ok(output)
    }
    /// Update the current resource on an already verified live scene.
    /// Other words, including the maximum, are preserved byte for byte.
    pub fn rebind_current_owner_resource(
        frame: &[u8],
        recorded: [u32; 2],
        current: u32,
    ) -> R<Vec<u8>> {
        let sections = Sections::parse(frame)?;
        let mut output = frame.to_vec();
        if let Some(existing) = sections.0.get("owner_resource") {
            if existing.bytes
                != recorded
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            {
                return Err("explicit source resource conflicts with recorded scene");
            }
            output[existing.offset..existing.offset + 4].copy_from_slice(&current.to_le_bytes());
        }
        Ok(output)
    }

    pub(crate) fn with_owner_resource(frame: &[u8], words: [u32; 2]) -> R<Vec<u8>> {
        let sections = Sections::parse(frame)?;
        if let Some(existing) = sections.0.get("owner_resource") {
            if existing.bytes
                != words
                    .into_iter()
                    .flat_map(u32::to_le_bytes)
                    .collect::<Vec<_>>()
            {
                return Err("explicit source resource conflicts with observed scene");
            }
            return Ok(frame.to_vec());
        }
        let mut output = frame.to_vec();
        let name = b"owner_resource";
        output.extend_from_slice(&(name.len() as u32).to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(&8u32.to_le_bytes());
        for word in words {
            output.extend_from_slice(&word.to_le_bytes());
        }
        Ok(output)
    }
    pub fn with_runtime_hit_inputs(frame: &[u8], hits: &[u8]) -> R<Vec<u8>> {
        let _ = Sections::parse(frame)?;
        crate::hit_buffer::HitBuffer::parse(
            std::str::from_utf8(hits).map_err(|_| "runtime hit inputs are not UTF8")?,
        )?;
        let mut output = Vec::with_capacity(frame.len() + hits.len() + 32);
        let mut at = 0;
        while at < frame.len() {
            let name_len = u32_at(frame, at) as usize;
            let name_start = at + 4;
            let size_at = name_start + name_len;
            let size = u32_at(frame, size_at) as usize;
            let end = size_at + 4 + size;
            if &frame[name_start..size_at] != b"runtime_hit_inputs" {
                output.extend_from_slice(&frame[at..end]);
            }
            at = end;
        }
        let name = b"runtime_hit_inputs";
        output.extend_from_slice(&(name.len() as u32).to_le_bytes());
        output.extend_from_slice(name);
        output.extend_from_slice(&(hits.len() as u32).to_le_bytes());
        output.extend_from_slice(hits);
        Ok(output)
    }

    pub(crate) fn append_hit_packet(&self, hit: usize, writes: &mut Vec<(u64, u32)>) {
        match self
            .hit_packets
            .get(hit)
            .copied()
            .unwrap_or(HitPacket::Prepared)
        {
            HitPacket::Prepared => {}
            HitPacket::UnimbuedOwnerProjectile { power, main_flags } => {
                writes.push((self.main_addr + 0x14, u32::MAX));
                writes.push((self.main_addr + 0x18, power));
                writes.push((self.main_addr + 0x298, main_flags));
            }
        }
    }

    /// Primary packet location in the immutable attack image.
    pub fn main_address(&self) -> u64 {
        self.main_addr
    }

    pub(crate) fn stat_address(&self) -> u64 {
        self.stat_addr
    }

    /// Normalized owner in the typed equipment producer's private namespace.
    /// Actual MAIN source ownership is independently bound through entity+3A0;
    /// projectile and observed process incarnations are separate identities.
    pub(crate) fn source_owner_address(&self) -> u64 {
        self.owner
    }

    pub(crate) fn uses_weapon_element(&self) -> bool {
        self.weapon_element_packet
    }

    pub(crate) fn damage_baseline(&self) -> (u64, std::ops::Range<usize>) {
        let offset = self.d_base_offset;
        (self.d_addr, offset..offset + self.d_base.len())
    }

    /// Player values may change, but they must target the same exported attack
    /// address space. A different scene/domain requires a matching attack bundle.
    pub(crate) fn same_layout(&self, other: &Self) -> bool {
        [
            self.stat_addr,
            self.d_addr,
            self.main_addr,
            self.ext_addr,
            self.owner,
            self.attr,
            self.dest,
        ] == [
            other.stat_addr,
            other.d_addr,
            other.main_addr,
            other.ext_addr,
            other.owner,
            other.attr,
            other.dest,
        ] && self.packet_kind == other.packet_kind
            && self.weapon_element_packet == other.weapon_element_packet
            && self.hit_packets == other.hit_packets
            && self.identity_ki_seed == other.identity_ki_seed
            && self.held_reaper == other.held_reaper
    }

    pub fn load(data: &[u8]) -> R<Frame> {
        let s = Sections::parse(data)?;
        let addresses = s.get("addresses")?;
        // Absence identifies the original canonical frame format. Current-frame
        // exporters explicitly store their own non-equipment defense baseline.
        let baseline_defense_sources = match s.0.get("baseline_sources") {
            None => LEGACY_BASE_DEFENSE_SOURCES,
            Some(section) if section.bytes.len() == 4 => u32_at(&section.bytes, 0),
            Some(_) => return Err("invalid baseline_sources section"),
        };
        let packet_kind = match (s.0.get("packet_kind"), s.0.get("aad_packet_kind")) {
            (None, None) => PacketKind::Weapon,
            (Some(section), None) => PacketKind::Fixed(u16::from_le_bytes(
                section
                    .bytes
                    .as_slice()
                    .try_into()
                    .map_err(|_| "invalid packet_kind section")?,
            )),
            (None, Some(section)) => match section.bytes.as_slice() {
                [kind] => PacketKind::DamageRow(*kind as i8),
                _ => return Err("invalid aad_packet_kind section"),
            },
            (Some(_), Some(_)) => return Err("conflicting packet kind sources"),
        };
        if addresses.len() < 9 * 8
            || s.get("stat")?.len() != 0x900
            || s.get("d0")?.len() != 0xB00
            || s.get("d_base")?.len() != 0xB00
            || s.get("s_base")?.len() < 0x7C0
        {
            return Err("frame sections have the wrong size");
        }
        let q =
            |i: usize| u64::from_le_bytes(addresses[i * 8..i * 8 + 8].try_into().unwrap_or([0; 8]));
        let weapon_element_packet = match s.0.get("weapon_element_packet") {
            None => false,
            Some(section) if section.bytes == [1] => true,
            Some(_) => return Err("invalid weapon_element_packet section"),
        };
        let hit_packets = match s.0.get("hit_packets") {
            None => Vec::new(),
            Some(section) => {
                let (records, tail) = section.bytes.as_chunks::<12>();
                if records.is_empty() || !tail.is_empty() {
                    return Err("invalid hit_packets section");
                }
                records
                    .iter()
                    .map(|row| match u32_at(row, 0) {
                        0 if u32_at(row, 4) == 0 => Ok(HitPacket::Prepared),
                        0 => Err("nonzero prepared hit packet word"),
                        1 => Ok(HitPacket::UnimbuedOwnerProjectile {
                            power: u32_at(row, 4),
                            main_flags: u32_at(row, 8),
                        }),
                        _ => Err("unknown hit packet mode"),
                    })
                    .collect::<R<Vec<_>>>()?
            }
        };
        let held_reaper = match s.0.get("held_reaper") {
            None => false,
            Some(section) if section.bytes == [1] => true,
            Some(_) => return Err("invalid held_reaper sequence policy"),
        };
        let identity_ki_seed = match s.0.get("identity_ki_seed") {
            None => false,
            // Version1, original376698 descriptor[8]=0, factor=1.0f32.
            Some(section)
                if section.bytes == [1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63] =>
            {
                true
            }
            Some(_) => return Err("invalid original world Ki seed contract"),
        };
        let d_base = s.0.get("d_base").ok_or("missing damage baseline")?;
        let (native_before, native_after) = match s.0.get("native_sources") {
            None => (Vec::new(), Vec::new()),
            Some(section) => {
                let bytes = &section.bytes;
                if bytes.len() < 12 || u32_at(bytes, 0) != 1 {
                    return Err("invalid native source program header");
                }
                let before = u32_at(bytes, 4) as usize;
                let after = u32_at(bytes, 8) as usize;
                if before > 4096
                    || after > 4096
                    || before + after > 4096
                    || bytes.len() != 12 + (before + after) * 8
                {
                    return Err("invalid native source program extent");
                }
                let entries = bytes[12..]
                    .as_chunks::<8>()
                    .0
                    .iter()
                    .map(|row| {
                        let group = u32_at(row, 0);
                        if !(1..=65535).contains(&group) {
                            return Err("invalid native source group");
                        }
                        Ok((group as u16, u32_at(row, 4)))
                    })
                    .collect::<R<Vec<_>>>()?;
                (entries[..before].to_vec(), entries[before..].to_vec())
            }
        };
        Ok(Frame {
            stat: s.get("stat")?.clone(),
            d0: s.get("d0")?.clone(),
            s_base: s.get("s_base")?.clone(),
            d_base: d_base.bytes.clone(),
            d_base_offset: d_base.offset,
            stat_addr: q(0),
            d_addr: q(1),
            main_addr: q(2),
            ext_addr: q(3),
            owner: q(4),
            owner_resource: match s.0.get("owner_resource") {
                None => None,
                Some(value) if value.bytes.len() == 8 => {
                    Some((u32_at(&value.bytes, 0), i32_at(&value.bytes, 4)))
                }
                Some(_) => return Err("invalid source owner_resource section"),
            },
            actor_agility: match s.0.get("actor_agility") {
                None => None,
                Some(value) if value.bytes.len() == 4 && value.bytes[3] <= 7 => Some(
                    value
                        .bytes
                        .as_slice()
                        .try_into()
                        .map_err(|_| "invalid actor agility extent")?,
                ),
                Some(_) => return Err("invalid actor agility section"),
            },
            native_before,
            native_after,
            source_rebuild: s
                .0
                .get("source_rebuild")
                .map(|section| source_rebuild::Program::load(&section.bytes))
                .transpose()?,
            source_baseline_ready: !s.0.contains_key("source_rebuild"),
            attr: q(5),
            dest: q(6),
            main8: q(7) as u32,
            packet_kind,
            hit_packets,
            runtime_hit_inputs: s
                .0
                .get("runtime_hit_inputs")
                .map(|section| section.bytes.clone()),
            weapon_element_packet,
            hp_base: q(8) as u32,
            baseline_defense_sources,
            held_reaper,
            identity_ki_seed,
        })
    }

    pub fn characters(data: &[u8]) -> R<Vec<Character>> {
        let s = Sections::parse(data)?;
        let mut out = Vec::new();
        while let Some(section) = s.0.get(&format!("char{}", out.len())) {
            let b = &section.bytes;
            if b.len() < 40 + 0x2018 {
                return Err("character section too short");
            }
            let w = words(&b[..40]);
            let (inv, rest) = b[40..].split_at(0x2018);
            let entries: Vec<Entry> = rest
                .as_chunks::<8>()
                .0
                .iter()
                .map(|e| Entry {
                    key: u16_at(e, 0),
                    dup: u16_at(e, 2),
                    base: u32_at(e, 4),
                })
                .collect();
            if entries.len() != w[8] as usize + w[9] as usize {
                return Err("character core counts do not match");
            }
            let (first, second) = entries.split_at(w[8] as usize);
            out.push(Character {
                inv: inv.to_vec(),
                mode: w[0] as i32,
                active: w[1] as usize,
                cap: w[2],
                plus_cap: w[3],
                style: w[4] as i32,
                unarmed: w[5],
                level: w[6],
                core_mode: w[7] as i32,
                core_maps: [first.to_vec(), second.to_vec()],
            });
        }
        Ok(out)
    }
}
impl Character {
    /// The record as the blacksmith can leave it: a levelled item at the level cap, and a + value raised to the
    /// character's cap. Items without a level (flag 0x200000 or level 1) and items without a + value are left as they
    /// are, since soul match does not give them one.
    pub(crate) fn enhanced(&self, item: &mut [u8; ITEM]) {
        let flags = u32_at(item, 0x18);
        let level = u16_at(item, 6);
        if flags & 0x200000 == 0 && level > 1 && level < LEVEL_CAP {
            item[6..8].copy_from_slice(&LEVEL_CAP.to_le_bytes());
        }
        let plus = u16_at(item, 0xA) as u32;
        if plus > 0 && plus < self.cap && self.cap <= u16::MAX as u32 {
            item[0xA..0xC].copy_from_slice(&(self.cap as u16).to_le_bytes());
        }
    }
}

#[cfg(test)]
pub(crate) fn audit_fixture() -> (Tables, Frame, Character) {
    audit_tests::fixture()
}

#[cfg(test)]
impl Tables {
    pub(crate) fn audit_set_reward_flags(&mut self, side: u32, row: usize, flags: u8) {
        assert!(row < 7);
        let index = self.sides.index(side).expect("fixture side must exist") as usize;
        self.sides.rows[8 + index * self.sides.stride + 0x34 + 24 * row] = flags;
    }
}

/// The independent PC202 replay supplies only (group, raw value, previous word)
/// and a digest of its full output banks. Run the actual production materializer.
#[cfg(test)]
#[test]
#[ignore = "Requires the PC202 set/skill dispatcher corpus and its bound tables.bin"]
fn set_and_skill_materializers_match_original_dispatch() {
    use sha2::{Digest, Sha256};
    use std::{fs, path::PathBuf};

    let dir = PathBuf::from(std::env::var_os("NIOH3_ADDITIVE_REFERENCE").unwrap());
    let table_path = std::env::var_os("NIOH3_MATERIALIZER_TABLES").unwrap();
    let table_bytes = fs::read(table_path).unwrap();
    let tables = Tables::load(&table_bytes).unwrap();
    let (_, _, character) = audit_fixture();
    let hash = format!("{:x}", Sha256::digest(&table_bytes));
    for family in ["D_ADD", "D_LITERAL"] {
        let cases = fs::read(dir.join(format!("{family}_INPUTS.bin"))).unwrap();
        let report_bytes = fs::read(dir.join(format!("{family}_ORIGINAL.json"))).unwrap();
        let report: serde_json::Value = serde_json::from_slice(&report_bytes).unwrap();
        assert!(report["native_inputs"]
            .as_object()
            .unwrap()
            .values()
            .any(|v| v.as_str() == Some(&hash)));
        let mut stat = [0u8; 0x900];
        let mut bank = [0u8; 0xb00];
        let mut result = Sha256::new();
        let (vectors, trailing) = cases.as_chunks::<10>();
        assert!(trailing.is_empty());
        let count: u64 = report["original_calls"]
            .as_object()
            .unwrap()
            .values()
            .map(|v| v.as_u64().unwrap())
            .sum();
        assert_eq!(vectors.len() as u64, count);
        for input in vectors {
            let group = u16_at(input, 0);
            for word in bank.as_chunks_mut::<4>().0 {
                word.copy_from_slice(&input[6..10]);
            }
            tables
                .materialize_one(
                    &character,
                    &[0; 7],
                    group,
                    u32_at(input, 2),
                    &mut stat,
                    &mut bank,
                )
                .unwrap();
            result.update(input);
            result.update(bank);
        }
        assert_eq!(
            format!("{:x}", result.finalize()),
            report["comparison_sha256"].as_str().unwrap(),
            "Native bank comparison for {family}"
        );
        assert_eq!(stat, [0; 0x900]);
    }
}

#[cfg(test)]
pub(crate) fn audit_frame_bytes() -> Vec<u8> {
    let (_, f, _) = audit_fixture();
    let mut data = Vec::new();
    let mut add = |name: &str, bytes: &[u8]| {
        data.extend_from_slice(&(name.len() as u32).to_le_bytes());
        data.extend_from_slice(name.as_bytes());
        data.extend_from_slice(&(bytes.len() as u32).to_le_bytes());
        data.extend_from_slice(bytes);
    };
    let addresses: Vec<u8> = [
        f.stat_addr,
        f.d_addr,
        f.main_addr,
        f.ext_addr,
        f.owner,
        f.attr,
        f.dest,
        u64::from(f.main8),
        u64::from(f.hp_base),
    ]
    .into_iter()
    .flat_map(u64::to_le_bytes)
    .collect();
    add("addresses", &addresses);
    for (name, bytes) in [
        ("stat", &f.stat),
        ("d0", &f.d0),
        ("s_base", &f.s_base),
        ("d_base", &f.d_base),
    ] {
        add(name, bytes);
    }
    add("baseline_sources", &0u32.to_le_bytes());
    add("weapon_element_packet", &[1]);
    add(
        "identity_ki_seed",
        &[1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 128, 63],
    );
    data
}
