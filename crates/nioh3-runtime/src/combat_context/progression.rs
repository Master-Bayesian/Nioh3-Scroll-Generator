//! PC202 normal-owner source compiler, progress branch of 556118.
//! Read only long-lived game state. A descriptor from an earlier rebuild's
//! stack is never available here; keep the skill projection explicitly named.
use super::*;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

const MANAGER: u64 = 0x45b9e30;
const COUNTERS: u64 = 0x4751848;
const FLAGS: u64 = 0x475a170;
const INHIBITOR: u64 = 0x4759910;
type R<T> = Result<T, RuntimeError>;

pub struct ActorBaseline {
    pub stat: Vec<u8>,
    pub materialized: Vec<u8>,
    pub reset_stat: Vec<u8>,
    pub reset_damage: Vec<u8>,
    pub health_seed: u32,
    pub primary_plus_bonus: u32,
}

pub fn unarmed_levels(memory: &dyn InventoryMemory, snap: &CombatSnapshot) -> R<[u32; 2]> {
    let read_inputs = || -> R<_> {
        let holder = q(memory, add(snap.module_base, 0x4751530)?)?;
        let data = q(memory, holder)?;
        let raw = read(memory, add(data, 0x2461b4)?, 8)?;
        Ok((data, [dword(&raw, 0), dword(&raw, 4)]))
    };
    let (data, levels) = read_inputs()?;
    if read_inputs()? != (data, levels)
        || memory.process() != snap.process
        || memory.recheck_creation_filetime()? != snap.process.creation_filetime
    {
        return Err(layout("unarmed level inputs changed during the read"));
    }
    Ok(levels)
}

/// For a selected STAT that requests synchronization, reproduce the ordinary
/// 552AAC guard. Network mode one also needs the peer-specific route. The
/// current normal-owner compiler has no evaluator for either synchronized route.
pub fn ordinary_equipment_branch(
    memory: &dyn InventoryMemory,
    snap: &CombatSnapshot,
    stat: &[u8],
) -> R<bool> {
    if stat.get(0x10) == Some(&0) {
        return Ok(true);
    }
    let network = q(memory, add(snap.module_base, 0x4709b50)?)?;
    if network != 0 && matches!(u(memory, add(network, 0x34)?)?, 1 | 2) {
        return Ok(false);
    }
    let holder = q(memory, add(snap.module_base, COUNTERS)?)?;
    let config = q(memory, add(holder, 8)?)?;
    Ok(read(memory, add(config, 0xcf8)?, 1)? == [0])
}

/// Rebuild the normal-owner reset state from the actual selected buffers.
/// No byte outside the original write footprint is filled from a template.
pub fn read_actor_baseline(
    memory: &dyn InventoryMemory,
    snap: &CombatSnapshot,
) -> R<ActorBaseline> {
    if memory.process() != snap.process || memory.module_base() != snap.module_base {
        return Err(layout("numeric inputs belong to another process"));
    }
    let read_once = || -> R<_> {
        let owner = snap.roots.owner;
        let flags = (
            u(memory, add(owner, 0x24)?)?,
            read(memory, add(owner, 0x3660)?, 1)?[0],
            read(memory, add(owner, 0x3663)?, 1)?[0],
        );
        if flags != (0, 0, 0) {
            return Err(layout("health reset requires another owner factor branch"));
        }
        let table = Parameters {
            memory,
            manager: q(memory, add(snap.module_base, MANAGER)?)?,
        }
        .table(0x90, 32)?;
        let mut health = 0u32;
        for (i, at) in [
            (snap.inputs.level as i32, 0),
            (snap.inputs.synchronized_attrs[0], 2),
            (snap.inputs.synchronized_attrs[2], 12),
        ] {
            let row = usize::try_from(i)
                .ok()
                .and_then(|idx| table.as_chunks::<32>().0.get(idx))
                .ok_or_else(|| layout("health input outside table90"))?;
            health += u32::from(word(row, at));
        }
        Ok((
            read(memory, snap.roots.stat, 0x900)?,
            read(memory, snap.roots.materializer, 0xb00)?,
            health,
        ))
    };
    let (stat, materialized, health_seed) = read_once()?;
    let (next_stat, next_damage, next_health) = read_once()?;
    if (stat.as_slice(), materialized.as_slice(), health_seed)
        != (next_stat.as_slice(), next_damage.as_slice(), next_health)
        || !matches!(roots(memory,snap.module_base)?,Observation::Value(now) if now==snap.roots)
        || memory.recheck_creation_filetime()? != snap.process.creation_filetime
        || dword(&stat, 0x24) != snap.inputs.stats.primary_attack
        || Some(dword(&stat, 0x28)) != snap.inputs.stats.secondary_attack
        || attrs(&stat, 0x844)? != snap.inputs.synchronized_attrs
    {
        return Err(layout("numeric input changed during first read"));
    }
    let (reset_stat, reset_damage) = super::reset::normal_owner(&stat, &materialized)?;
    let primary_plus_bonus = dword(&stat, 0x24).wrapping_add(dword(&stat, 0x2c));
    Ok(ActorBaseline {
        stat,
        materialized,
        reset_stat,
        reset_damage,
        health_seed,
        primary_plus_bonus,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Submission {
    pub group: u16,
    pub value: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgressionSources {
    pub elixir: Vec<Submission>,
    pub region: Vec<Submission>,
    pub clan: Vec<Submission>,
    pub titles: Vec<Submission>,
    pub completion: Vec<Submission>,
    pub minimum: Vec<Submission>,
    pub variants: Vec<Submission>,
    pub configured_progress: Vec<Submission>,
    pub slotted_progress: Vec<Submission>,
    pub late_clan: [Submission; 4],
    pub core_unlock_thresholds: [i32; 8],
    pub post_record_present: [[bool; 4]; 2],
    pub rule_table_digest: String,
}

impl ProgressionSources {
    pub fn layers(&self) -> [&[Submission]; 9] {
        [
            &self.elixir,
            &self.region,
            &self.clan,
            &self.titles,
            &self.completion,
            &self.minimum,
            &self.variants,
            &self.configured_progress,
            &self.slotted_progress,
        ]
    }
    /// Each submission is a fixed-width native group/value pair. Preserve
    /// duplicates, zero-valued submissions and family boundaries.
    pub fn digest(&self) -> String {
        let mut h = Sha256::new();
        h.update(b"nioh3-progress-sources/v1");
        for layer in self.layers().into_iter().chain([self.late_clan.as_slice()]) {
            h.update((layer.len() as u64).to_le_bytes());
            for entry in layer {
                h.update(entry.group.to_le_bytes());
                h.update(entry.value.to_le_bytes());
            }
        }
        for threshold in self.core_unlock_thresholds {
            h.update(threshold.to_le_bytes());
        }
        for bank in self.post_record_present {
            h.update(bank.map(u8::from));
        }
        h.update(self.rule_table_digest.as_bytes());
        format!("{:x}", h.finalize())
    }
}

fn word(row: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([row[at], row[at + 1]])
}
fn dword(row: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([row[at], row[at + 1], row[at + 2], row[at + 3]])
}

struct Parameters<'a> {
    memory: &'a dyn InventoryMemory,
    manager: u64,
}

impl Parameters<'_> {
    fn core_thresholds(&self) -> R<[i32; 8]> {
        let ctx = q(self.memory, add(self.manager, 0x720)?)?;
        let header = read(self.memory, ctx, 0x30)?;
        let extent = |offset, stride| -> R<(u64, usize)> {
            let start = u64_at(&header, offset)?;
            let end = u64_at(&header, offset + 8)?;
            let capacity = u64_at(&header, offset + 16)?;
            let size = end
                .checked_sub(start)
                .filter(|size| {
                    start != 0 && end <= capacity && size % stride == 0 && *size <= 4096 * stride
                })
                .ok_or_else(|| layout("invalid core unlock curve extent"))?;
            Ok((start, size as usize))
        };
        let (base, bytes) = extent(0, 12)?;
        let (index, indices) = extent(24, 8)?;
        if bytes / 12 != indices / 8 {
            return Err(layout("core curve index length differs"));
        }
        let rows = read(self.memory, base, bytes)?;
        let pointers = read(self.memory, index, indices)?;
        let mut curve = Vec::new();
        let mut previous = None;
        let mut visited = BTreeSet::new();
        for at in (0..indices).step_by(8) {
            let ptr = u64_at(&pointers, at)?;
            let offset = ptr
                .checked_sub(base)
                .filter(|n| *n < bytes as u64 && n % 12 == 0)
                .ok_or_else(|| layout("core curve index outside row storage"))?
                as usize;
            let row = &rows[offset..offset + 12];
            let key = (word(row, 8), dword(row, 0) as i32);
            if !visited.insert(offset) || previous.is_some_and(|prev| prev > key) {
                return Err(layout("core curve pointer permutation differs"));
            }
            previous = Some(key);
            if key.0 == 0xa865 {
                let value = f32::from_bits(dword(row, 4));
                if !value.is_finite() {
                    return Err(layout("nonfinite core unlock threshold"));
                }
                curve.push((key.1, value, row[10]));
            }
        }
        Ok(std::array::from_fn(|slot| {
            if slot < 2 || curve.is_empty() {
                return 0;
            }
            let x = slot as i32;
            let i = curve.partition_point(|p| p.0 <= x);
            if i == 0 {
                return curve[0].1 as i32;
            }
            let (lo, y, _) = curve[i - 1];
            match curve.get(i) {
                Some(&(hi, z, kind)) if kind != 0 => {
                    let fraction = ((x.wrapping_sub(lo)) as f32 / (hi.wrapping_sub(lo)) as f32)
                        .clamp(0.0, 1.0);
                    (fraction * (z - y) + y) as i32
                }
                _ => y as i32,
            }
        }))
    }
    fn table(&self, offset: u64, width: usize) -> R<Vec<u8>> {
        let ctx = q(self.memory, add(self.manager, offset)?)?;
        let payload = q(self.memory, ctx)?;
        let count = u(self.memory, add(payload, 4)?)? as usize;
        if count > 4096 {
            return Err(layout(
                "progress parameter row count exceeds the PC202 table bound",
            ));
        }
        let size = count * width;
        let mut rows = Vec::with_capacity(size);
        while rows.len() < size {
            let n = (size - rows.len()).min(4096);
            rows.extend(read(self.memory, add(payload, 8 + rows.len() as u64)?, n)?);
        }
        Ok(rows)
    }
    fn mode(&self, key: u8) -> R<u8> {
        let rows = self.table(0x260, 0xd0)?;
        let ctx = q(self.memory, add(self.manager, 0x260)?)?;
        let header = q(self.memory, add(ctx, 0x20)?)?;
        let empty = read(self.memory, add(header, 4)?, 1)?[0];
        let begin = q(self.memory, add(header, 8)?)?;
        let end = q(self.memory, add(header, 16)?)?;
        let size = end
            .checked_sub(begin)
            .filter(|n| *n > 0 && *n <= 65536 && n % 8 == 0)
            .ok_or_else(|| layout("invalid mode table bucket extent"))? as usize;
        let buckets = read(self.memory, begin, size)?;
        let k = u32::from(key);
        let mix = (k * 9) ^ ((k * 9) >> 11);
        let start = (mix.wrapping_shl(15).wrapping_add(mix) & 0x7fffffff) as usize % (size / 8);
        for probe in 0..size / 8 {
            let at = (start + probe) % (size / 8) * 8;
            if buckets[at] == key {
                let idx = dword(&buckets, at + 4) as usize;
                let over = rows
                    .as_chunks::<0xd0>()
                    .0
                    .get(idx)
                    .map(|r| r[0xcb])
                    .unwrap_or(0);
                return Ok(if over == 0 { key } else { over });
            }
            if buckets[at] == empty {
                return Ok(key);
            }
        }
        Err(layout("unterminated mode parameter lookup"))
    }
}

/// Validate membership using the same keyed list and bucket identities as the
/// native container. Its complete list alone does not establish native lookup.
fn graph(
    memory: &dyn InventoryMemory,
    address: u64,
    width: usize,
    key_bytes: usize,
) -> R<BTreeMap<u32, Vec<u8>>> {
    let head = read(memory, address, 64)?;
    let sentinel = u64_at(&head, 8)?;
    let count = u64_at(&head, 16)?;
    let begin = u64_at(&head, 24)?;
    let end = u64_at(&head, 32)?;
    let mask = u64_at(&head, 48)?;
    let len = end
        .checked_sub(begin)
        .ok_or_else(|| layout("invalid progress bucket span"))?;
    if count > 4096
        || mask >= 16384
        || !(mask + 1).is_power_of_two()
        || len != 16 * (mask + 1)
        || sentinel == 0
    {
        return Err(layout("invalid progress graph geometry"));
    }
    let buckets = read(memory, begin, len as usize)?;
    let sent = read(memory, sentinel, 16)?;
    let mut at = u64_at(&sent, 0)?;
    let mut prev = sentinel;
    let mut nodes = BTreeMap::new();
    let mut ranges = BTreeMap::<usize, (u64, u64)>::new();
    for _ in 0..count {
        if at == 0 || at == sentinel {
            return Err(layout("short progress list"));
        }
        let row = read(memory, at, width)?;
        if u64_at(&row, 8)? != prev {
            return Err(layout("progress backlink changed"));
        }
        let key = match key_bytes {
            1 => u32::from(row[16]),
            2 => u32::from(word(&row, 16)),
            4 => dword(&row, 16),
            _ => return Err(layout("invalid progress key width")),
        };
        let hash = key.to_le_bytes()[..key_bytes]
            .iter()
            .fold(0xcbf29ce484222325u64, |h, b| {
                (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
            });
        let bucket = (hash & mask) as usize;
        if let Some((_, tail)) = ranges.get_mut(&bucket) {
            if *tail != prev {
                return Err(layout("noncontiguous progress bucket"));
            }
            *tail = at;
        } else {
            ranges.insert(bucket, (at, at));
        }
        prev = at;
        at = u64_at(&row, 0)?;
        if nodes.insert(key, row).is_some() {
            return Err(layout("duplicate progress key"));
        }
    }
    if at != sentinel || u64_at(&sent, 8)? != prev {
        return Err(layout("progress list extent changed"));
    }
    for (index, pair) in buckets.as_chunks::<16>().0.iter().enumerate() {
        let expected = ranges.get(&index).copied().unwrap_or((sentinel, sentinel));
        if (u64_at(pair, 0)?, u64_at(pair, 8)?) != expected {
            return Err(layout("native progress bucket disagrees with list"));
        }
    }
    Ok(nodes)
}

fn push(out: &mut Vec<Submission>, group: u16, value: u32) {
    if group != 0 {
        out.push(Submission { group, value });
    }
}

fn ranks(
    rows: &[u8],
    width: usize,
    nodes: &BTreeMap<u32, Vec<u8>>,
    elixir: bool,
    profile: &[u8],
    mode: u8,
    flags: u64,
) -> R<Vec<Submission>> {
    let mut out = Vec::new();
    for row in rows.chunks_exact(width) {
        if elixir
            && ((row[16] & 1 == 0 && profile[0x44] < 1)
                || (row[16] & 2 != 0 && flags & (1 << 8) == 0)
                || (row[16] & 4 != 0 && flags & (1 << 9) == 0))
        {
            continue;
        }
        let node = nodes
            .get(&dword(row, 8))
            .ok_or_else(|| layout("native progression rank insertion unresolved"))?;
        let rank = dword(node, 20) as usize;
        if rank == 0 || (!elixir && row[20] != mode) {
            continue;
        }
        let (group, value) = if elixir { (0xc, 0x20) } else { (0x18, 0x28) };
        out.push(Submission {
            group: word(row, group),
            value: if rank <= 9 {
                dword(row, value + 8 * rank)
            } else {
                0
            },
        });
    }
    Ok(out)
}

type AreaLayers = (Vec<Submission>, Vec<Submission>, Vec<Submission>);
fn area_sources(tables: &Parameters<'_>, counter: u64, profile: u8) -> R<AreaLayers> {
    let counts = graph(tables.memory, add(counter, 0x388)?, 24, 4)?;
    let snapshots = graph(tables.memory, add(counter, 0x348)?, 0x38, 1)?;
    let mut minimums = BTreeMap::<u8, usize>::new();
    let mut completion = Vec::new();
    for row in tables.table(0x278, 0xd8)?.as_chunks::<0xd8>().0.iter() {
        let count = counts
            .get(&u32::from(word(row, 0x88)))
            .map_or(0, |n| dword(n, 20));
        let tier = (1..=4)
            .rev()
            .find(|&i| count >= dword(row, 0x30 + i * 4))
            .unwrap_or(0);
        minimums
            .entry(row[0x8a])
            .and_modify(|minimum| *minimum = (*minimum).min(tier))
            .or_insert(tier);
        for i in 1..=tier {
            push(
                &mut completion,
                word(row, 0x92 + i * 8),
                u32::from(word(row, 0x94 + i * 8)),
            );
        }
    }
    let rows = tables.table(0x268, 0x38)?;
    let mut minimum = Vec::new();
    let mut variants = Vec::new();
    for row in rows.as_chunks::<0x38>().0.iter() {
        if row[16] != 0 {
            for i in 0..minimums.get(&row[16]).copied().unwrap_or(0) {
                push(
                    &mut minimum,
                    word(row, 0x12 + i * 6),
                    u32::from(word(row, 0x14 + i * 6)),
                );
            }
        }
    }
    if profile >= 6 {
        return Err(layout("unsupported area profile selector"));
    }
    for variant in 2..=usize::from(profile) {
        for row in rows.as_chunks::<0x38>().0.iter() {
            let count = snapshots
                .get(&u32::from(row[16]))
                .map_or(0, |n| dword(n, 20 + 4 * variant));
            let i = variant - 2;
            if variant <= 4 && count >= dword(row, 4 + 4 * i) {
                push(
                    &mut variants,
                    word(row, 0x2a + 4 * i),
                    u32::from(word(row, 0x2c + 4 * i)),
                );
            }
        }
    }
    Ok((completion, minimum, variants))
}

const CATEGORIES: [u32; 16] = [
    0x1909, 0x5fff, 0x6e73, 0x5455, 0x5095, 0x1c17, 0x2d3f, 0x72b1, 0x5e1b, 0x027c, 0x0d2f, 0x17d6,
    0x04e6, 0x2552, 0x1302, 0x33c9,
];
fn skill_sources(
    tables: &Parameters<'_>,
    snapshot: &CombatSnapshot,
    category: u32,
    slots: &[u8],
) -> R<(Vec<Submission>, Vec<Submission>)> {
    let defs = tables.table(0x130, 192)?;
    let params = tables.table(0x598, 148)?;
    let by_key: BTreeMap<_, _> = defs
        .as_chunks::<192>()
        .0
        .iter()
        .map(|r| (word(r, 24), r))
        .collect();
    let effects: BTreeMap<_, _> = params
        .as_chunks::<148>()
        .0
        .iter()
        .map(|r| ((word(r, 0), dword(r, 4)), r))
        .collect();
    if by_key.len() != defs.len() / 192 || effects.len() != params.len() / 148 {
        return Err(layout("ambiguous skill definition"));
    }
    let configured: BTreeMap<_, _> = snapshot
        .inputs
        .configured_skills
        .iter()
        .map(|s| (s.key, s))
        .collect();
    let native = graph(tables.memory, add(snapshot.roots.progress, 0x6020)?, 32, 2)?;
    if native.len() != configured.len()
        || configured.iter().any(|(key, s)| {
            !native
                .get(&u32::from(*key))
                .is_some_and(|n| word(n, 0x1a) == s.rank && word(n, 0x1c) == s.enabled)
        })
    {
        return Err(layout(
            "configured ranks no longer match their native buckets",
        ));
    }
    let selected = CATEGORIES.iter().position(|v| *v == category);
    let emit = |out: &mut Vec<Submission>, key, aux| {
        let rank = configured.get(&key).map_or(1, |s| u32::from(s.rank));
        if let Some(row) = effects.get(&(key, rank)) {
            if aux && word(*row, 0x64) == 0 {
                return;
            }
            let offsets: &[usize] = if aux {
                &[0x64, 0x70]
            } else {
                &[0x40, 0x4c, 0x58]
            };
            for &at in offsets {
                push(out, word(*row, at), dword(*row, at + 8));
            }
        }
    };
    let mut learned = Vec::new();
    for row in defs.as_chunks::<192>().0.iter() {
        let key = word(row, 24);
        if !configured.get(&key).is_some_and(|s| s.enabled != 0) {
            continue;
        }
        let c = i32::from(row[0xac] as i8);
        let allowed = match c {
            23 => true,
            21 => snapshot.inputs.mode == 0,
            22 => snapshot.inputs.mode == 1,
            _ => c == selected.map_or(-1, |v| v as i32) || row[0x1c] & 0x40 != 0,
        };
        if row[0xb9] == 0 || (row[0xb9] == 1 && row[0x1c] & 1 == 0 && allowed) {
            emit(&mut learned, key, row[0xb9] == 0);
        }
    }
    let mut slotted = Vec::new();
    for i in selected.into_iter().chain([21, 22, 23]) {
        let key = word(slots, i * 8);
        if key != 0 && by_key.get(&key).is_some_and(|r| r[0xb9] == 1) {
            emit(&mut slotted, key, false);
        }
    }
    Ok((learned, slotted))
}

fn post_selection(memory: &dyn InventoryMemory, snap: &CombatSnapshot) -> R<[[bool; 4]; 2]> {
    let holder = q(memory, add(snap.module_base, 0x4751530)?)?;
    let data = q(memory, holder)?;
    let selectors = read(memory, add(data, 0x23b630)?, 0x88)?;
    let regular = q(memory, add(data, 0x927d0)?)?;
    let special = q(memory, add(data, 0x23b4e0)?)?;
    if regular > 2500 || special > 400 {
        return Err(layout("invalid post-selection inventory extents"));
    }
    let mut present = [[false; 4]; 2];
    for (bank, entries) in present.iter_mut().enumerate() {
        for (entry, slot) in entries.iter_mut().zip([13usize, 9, 10, 15]) {
            let index = u64::from(dword(&selectors, 0x44 * bank + 4 * slot));
            let (count, offset, stride) = if slot == 15 {
                (special, 0x224a60, 0xe8)
            } else {
                (regular, 0x10, 0xf0)
            };
            if index < count {
                let raw = read(memory, add(data, offset + stride * index)?, 0xe8)?;
                let start = bank * BANK_BYTES + slot * 0xe8;
                if snap.inputs.inventory.get(start..start + 0xe8) != Some(raw.as_slice()) {
                    return Err(layout("post-selection item differs from the current bank"));
                }
                *entry = true;
            }
        }
    }
    Ok(present)
}

fn collect(
    memory: &dyn InventoryMemory,
    snap: &CombatSnapshot,
    category: u32,
) -> R<ProgressionSources> {
    let base = snap.module_base;
    let progress = snap.roots.progress;
    let tables = Parameters {
        memory,
        manager: q(memory, add(base, MANAGER)?)?,
    };
    let ranks_a = graph(memory, add(progress, 0x5d98)?, 24, 4)?;
    let ranks_b = graph(memory, add(progress, 0x5de8)?, 24, 4)?;
    let title_nodes = graph(memory, add(progress, 0x5ea8)?, 28, 2)?;
    let profile = read(memory, snap.roots.profile, PROFILE_BYTES)?;
    let f = q(memory, add(base, FLAGS)?)?;
    let inhibit = q(memory, add(base, INHIBITOR)?)?;
    let flags = if f != 0 && (inhibit == 0 || read(memory, add(inhibit, 0x3c)?, 1)?[0] == 0) {
        q(memory, add(f, 8)?)?
    } else {
        0
    };
    let mode = tables.mode(profile[0x11])?;
    let elixir = ranks(
        &tables.table(0x7c0, 0x70)?,
        0x70,
        &ranks_a,
        true,
        &profile,
        mode,
        flags,
    )?;
    let region = ranks(
        &tables.table(0x7c8, 0x78)?,
        0x78,
        &ranks_b,
        false,
        &profile,
        mode,
        flags,
    )?;
    let raw = read(memory, add(progress, 0x6340)?, 64)?;
    let late_clan = std::array::from_fn(|i| Submission {
        group: word(&raw, 16 * i),
        value: dword(&raw, 16 * i + 4),
    });
    let school = usize::try_from(snap.inputs.mode)
        .ok()
        .filter(|s| *s < 2)
        .ok_or_else(|| layout("unsupported source compiler form"))?;
    let clan = late_clan[school * 2..school * 2 + 2]
        .iter()
        .filter(|s| s.group != 0)
        .copied()
        .collect();
    let mut titles = Vec::new();
    for row in tables.table(0x7e0, 0x110)?.as_chunks::<0x110>().0.iter() {
        let node = title_nodes
            .get(&u32::from(word(row, 0x80)))
            .ok_or_else(|| layout("native title rank insertion unresolved"))?;
        let rank = dword(node, 24) as usize;
        if rank != 0 {
            titles.push(Submission {
                group: word(row, 0x88),
                value: if rank <= 30 {
                    dword(row, 4 + rank * 4)
                } else {
                    0
                },
            });
        }
    }
    let counter = q(memory, add(q(memory, add(base, COUNTERS)?)?, 8)?)?;
    let (completion, minimum, variants) = area_sources(&tables, counter, profile[0x45])?;
    let slots = read(memory, add(progress, 0x60a0)?, 24 * 8)?;
    let (configured_progress, slotted_progress) = skill_sources(&tables, snap, category, &slots)?;
    let mut parameter_hash = Sha256::new();
    for (offset, width) in [(0x3f0, 256), (0x408, 64), (0x118, 84)] {
        let rows = tables.table(offset, width)?;
        parameter_hash.update((rows.len() as u64).to_le_bytes());
        parameter_hash.update(rows);
    }
    Ok(ProgressionSources {
        elixir,
        region,
        clan,
        titles,
        completion,
        minimum,
        variants,
        configured_progress,
        slotted_progress,
        late_clan,
        core_unlock_thresholds: tables.core_thresholds()?,
        post_record_present: post_selection(memory, snap)?,
        rule_table_digest: format!("{:x}", parameter_hash.finalize()),
    })
}

/// Derive the ordered progress branch from a verified PC202 role. The caller
/// owns the selected weapon's native category. Completion of a whole rebuild
/// additionally requires the transient compiler descriptor and reset inputs.
pub fn read_progression(
    memory: &dyn InventoryMemory,
    snapshot: &CombatSnapshot,
    category: u32,
) -> R<ProgressionSources> {
    if memory.process() != snapshot.process || memory.module_base() != snapshot.module_base {
        return Err(layout("progress inputs belong to another process"));
    }
    let first = collect(memory, snapshot, category)?;
    let second = collect(memory, snapshot, category)?;
    let still_current = matches!(read_combat_snapshot(memory)?, SnapshotOutcome::Ready(now)
        if now.process==snapshot.process && now.roots==snapshot.roots && now.inputs==snapshot.inputs);
    if first != second || !still_current {
        return Err(layout("progression changed during the source read"));
    }
    Ok(second)
}
