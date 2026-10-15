//! PC v2.02 player resources. Consumer evidence and validation scope are described
//! in docs/product/PLAYER_EDITOR_VALIDATION.md. No game function is invoked.
use crate::character::{locate_player, LiveEditOutcome};
use crate::error::RuntimeError;
use crate::inventory::InventoryMemory;
use crate::mutation::memory::TargetProcess;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const INGOT_MAX: u64 = 9_999_999_999_999;
pub const FRAGMENTS_MAX: u64 = 999_999_999;
pub const GLORY_MAX: u64 = 2_000_000_000;
pub const ACCOUNT_RVA: u64 = 0x45b9e70;
pub const THRESHOLDS: [u64; 10] = [
    0, 1000, 3000, 6000, 10000, 15000, 21000, 30000, 42500, 60000,
];
// Exact native consumers: stored-Amrita clamp, fragment clamp, Glory mirror,
// clan contribution lookup/rank calculation and the independent player wrapper.
pub const SITES: &[(u64, &[u8])] = &[
    (
        0x217f330,
        &[0x49, 0xb8, 0xff, 0x9f, 0x72, 0x4e, 0x18, 0x09, 0, 0],
    ),
    (0x217f34b, &[0x48, 0x8b, 0x01, 0x4c, 0x89, 0x40, 0x30, 0xc3]),
    (0x2154031, &[0xb9, 0xff, 0xc9, 0x9a, 0x3b]),
    (0x2154048, &[0x89, 0x4a, 0x20, 0xc3]),
    (0x220a15a, &[0xb8, 0, 0x94, 0x35, 0x77]),
    (
        0x220a1ec,
        &[
            0x41, 0x8b, 0x86, 0x74, 0x12, 0x02, 0, 0x89, 0x81, 0xdc, 0x62, 0, 0,
        ],
    ),
    (0x2935edd, &[0x4c, 0x8d, 0x81, 0x80, 0x13, 0x02, 0]),
    (0x2935f41, &[0x48, 0x8b, 0x87, 0x80, 0x12, 0x02, 0]),
    (0x2935f79, &[0x48, 0x8b, 0x46, 0x10, 0x48, 0x03, 0xc3]),
    (0x1e1322e, &[0x48, 0x8b, 0x10, 0x48, 0x63, 0x42, 0x20]),
    (0x1e3b84c, &[0x48, 0x8b, 0x5f, 0x30]),
    (0x5a7064, &[0x48, 0x8b, 0x05, 0x8d, 0x14, 0x02, 0x04]),
    (0x5a7009, &[0x48, 0x6b, 0xc8, 0x64, 0x48, 0x2b, 0x4b, 0x08]),
    (
        0x2935f50,
        &[0x48, 0xb8, 0xdb, 0x34, 0xb6, 0xd7, 0x82, 0xde, 0x1b, 0x43],
    ),
];
fn fail(detail: &str) -> RuntimeError {
    RuntimeError::InventoryChain {
        detail: format!("player resources: {detail}"),
    }
}
fn bytes(m: &dyn InventoryMemory, at: u64, n: usize) -> Result<Vec<u8>, RuntimeError> {
    let data = m.read(at, n)?;
    if data.len() != n {
        return Err(fail("short read"));
    }
    Ok(data)
}
fn u32_at(m: &dyn InventoryMemory, at: u64) -> Result<u32, RuntimeError> {
    Ok(u32::from_le_bytes(
        bytes(m, at, 4)?.try_into().map_err(|_| fail("u32"))?,
    ))
}
fn u64_at(m: &dyn InventoryMemory, at: u64) -> Result<u64, RuntimeError> {
    Ok(u64::from_le_bytes(
        bytes(m, at, 8)?.try_into().map_err(|_| fail("u64"))?,
    ))
}
fn pointer(m: &dyn InventoryMemory, at: u64) -> Result<u64, RuntimeError> {
    let value = u64_at(m, at)?;
    if !(0x10000..=0x7fff_ffff_fff8).contains(&value) || value % 8 != 0 {
        return Err(fail("invalid pointer"));
    }
    Ok(value)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clan {
    pub id: u32,
    pub node: u64,
    pub stored: u64,
    pub tenure: u64,
}
impl Clan {
    pub fn rank(&self) -> usize {
        rank(self.stored.saturating_add(self.tenure))
    }
}
pub fn rank(effective: u64) -> usize {
    THRESHOLDS
        .iter()
        .rposition(|v| effective >= *v)
        .unwrap_or(0)
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Snapshot {
    pub pid: u32,
    pub birth: u64,
    pub owner: u64,
    wrapper: u64,
    account: u64,
    pub ingot: u64,
    pub fragments: u64,
    pub glory: u64,
    pub current_clan: u32,
    joined: u64,
    sequence: u64,
    player_sequence: u64,
    pub clans: Vec<Clan>,
    ingot_owned: bool,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Patch {
    pub address: u64,
    pub before: Vec<u8>,
    pub after: Vec<u8>,
}
impl Snapshot {
    pub fn id(&self) -> String {
        format!("{:x}", Sha256::digest(format!("{self:?}")))
    }
    pub fn json(&self) -> Value {
        json!({"player_resources":{"process_id":self.pid,"process_birth":self.birth.to_string(),
        "snapshot_id":self.id(),"experimental":true,"ingot_owned":self.ingot_owned,
        "ingot":self.ingot.to_string(),"ingot_max":INGOT_MAX.to_string(),
        "fragments":self.fragments.to_string(),"fragments_max":FRAGMENTS_MAX.to_string(),
        "glory":self.glory.to_string(),"glory_max":GLORY_MAX.to_string(),
        "glory_editable":self.sequence == self.player_sequence,
        "current_clan_id":self.current_clan,
        "clans":self.clans.iter().map(|c|json!({"clan_id":c.id,"rank":c.rank(),
            "stored":c.stored.to_string(),"tenure":c.tenure.to_string()})).collect::<Vec<_>>()}})
    }
    pub fn plan(
        &self,
        field: &str,
        clan: u32,
        value: u64,
    ) -> Result<(Self, Vec<Patch>), RuntimeError> {
        let mut next = self.clone();
        let mut patches = Vec::new();
        let mut push = |address: u64, before: u64, after: u64, width: usize| {
            patches.push(Patch {
                address,
                before: before.to_le_bytes()[..width].to_vec(),
                after: after.to_le_bytes()[..width].to_vec(),
            });
        };
        if field != "clan_rank" && clan != 0 {
            return Err(fail("unexpected clan selector"));
        }
        match field {
            "ingot" if value <= INGOT_MAX && self.ingot_owned => {
                push(self.owner + 0x30, self.ingot, value, 8);
                next.ingot = value;
            }
            "fragments" if value <= FRAGMENTS_MAX => {
                push(self.owner + 0x20, self.fragments, value, 4);
                next.fragments = value;
            }
            "glory" if value <= GLORY_MAX => {
                if self.sequence != self.player_sequence {
                    return Err(fail("Glory synchronization markers differ; use a normal game gain or purchase and reload"));
                }
                // Keep the account balance and serialized player mirror equal.
                // Do not synthesize faction scores, network events or sequence IDs.
                push(self.account + 0x21274, self.glory, value, 4);
                push(self.owner + 0x62dc, self.glory, value, 4);
                next.glory = value;
            }
            "clan_rank" if value < 10 => {
                let c = next
                    .clans
                    .iter_mut()
                    .find(|c| c.id == clan)
                    .ok_or_else(|| fail("clan has no existing contribution record"))?;
                let target = THRESHOLDS[value as usize].saturating_sub(c.tenure);
                if rank(target.saturating_add(c.tenure)) != value as usize {
                    return Err(fail("tenure prevents this rank"));
                }
                if c.rank() == value as usize {
                    return Err(fail("rank already matches"));
                }
                push(c.node + 0x10, c.stored, target, 8);
                c.stored = target;
            }
            _ => {
                return Err(fail(
                    "unsupported field, unavailable record or value outside native range",
                ))
            }
        }
        if patches.iter().all(|p| p.before == p.after) {
            return Err(fail("nothing to change"));
        }
        Ok((next, patches))
    }
    pub fn value(&self, field: &str, clan: u32) -> Result<u64, RuntimeError> {
        match field {
            "ingot" => Ok(self.ingot),
            "fragments" => Ok(self.fragments),
            "glory" => Ok(self.glory),
            "clan_rank" => self
                .clans
                .iter()
                .find(|c| c.id == clan)
                .map(|c| c.rank() as u64)
                .ok_or_else(|| fail("missing clan")),
            _ => Err(fail("unknown field")),
        }
    }
}
// Mirrors 0x2935F0C exactly; keep raw clock units rather than assume a duration.
pub fn tenure_credit(now: u64, joined: u64) -> Result<u64, RuntimeError> {
    let delta = now.saturating_sub(joined);
    let first = ((u128::from(delta) * 0x431bde82d7b634dbu128) >> 82) as u64;
    let second = ((u128::from(first) * 0xc22e450672894ab7u128) >> 80) as u64;
    second
        .checked_mul(200)
        .ok_or_else(|| fail("tenure overflow"))
}
#[cfg(windows)]
fn clock(m: &dyn InventoryMemory) -> Result<u64, RuntimeError> {
    use windows_sys::Win32::System::Performance::{
        QueryPerformanceCounter, QueryPerformanceFrequency,
    };
    let mut counter = 0;
    let mut frequency = 0;
    let root = pointer(m, m.module_base() + 0x45c84f8)?;
    let clock = pointer(m, root + 0x6a8)?;
    let before = bytes(m, clock, 16)?;
    if unsafe { QueryPerformanceCounter(&mut counter) } == 0
        || unsafe { QueryPerformanceFrequency(&mut frequency) } == 0
        || frequency != 10_000_000
    {
        return Err(fail("unsupported native clock frequency"));
    }
    if bytes(m, clock, 16)? != before {
        return Err(fail("native clock changed while reading"));
    }
    let anchor = i64::from_le_bytes(before[8..16].try_into().map_err(|_| fail("clock anchor"))?);
    let base = u32::from_le_bytes(before[..4].try_into().map_err(|_| fail("clock base"))?);
    if counter < 0 || i64::try_from(i128::from(counter) * 100 - i128::from(anchor)).is_err() {
        return Err(fail("native clock arithmetic outside bounds"));
    }
    let seconds =
        (i128::from(counter) * 100 - i128::from(anchor)) / 1_000_000_000 + i128::from(base);
    u64::try_from(seconds).map_err(|_| fail("native clock outside bounds"))
}
#[cfg(not(windows))]
fn clock(_: &dyn InventoryMemory) -> Result<u64, RuntimeError> {
    Err(fail("Windows clock required"))
}

fn once(m: &dyn InventoryMemory, now: u64) -> Result<Snapshot, RuntimeError> {
    for (rva, expected) in SITES {
        if bytes(m, m.module_base() + rva, expected.len())? != *expected {
            return Err(fail("native signature changed"));
        }
    }
    for (index, threshold) in THRESHOLDS.iter().enumerate() {
        if u64_at(m, m.module_base() + 0x3bdcb30 + index as u64 * 8)? != *threshold {
            return Err(fail("native rank thresholds changed"));
        }
    }
    let owner = locate_player(m)?;
    let wrapper = pointer(m, m.module_base() + crate::prestige::POINTER_RVA)?;
    if pointer(m, wrapper)? != owner {
        return Err(fail("player wrapper ownership differs"));
    }
    let account = pointer(m, m.module_base() + ACCOUNT_RVA)?;
    let ingot = u64_at(m, owner + 0x30)?;
    let fragments = u64::from(u32_at(m, owner + 0x20)?);
    let glory = u64::from(u32_at(m, account + 0x21274)?);
    let current_clan = u32_at(m, account + 0x21220)?;
    let joined = u64_at(m, account + 0x21280)?;
    let sequence = u64_at(m, account + 0x21268)?;
    for (valid, detail) in [
        (ingot <= INGOT_MAX, "stored Amrita exceeds native range"),
        (
            fragments <= FRAGMENTS_MAX,
            "Soul Fragments exceed native range",
        ),
        (glory <= GLORY_MAX, "Glory exceeds native range"),
        (current_clan <= 50, "unknown current clan"),
        (
            u32_at(m, owner + 0x62dc)? as u64 == glory,
            "Glory account/player mirror differs",
        ),
        (
            u32_at(m, owner + 0x62e0)? == current_clan,
            "current-clan account/player mirror differs",
        ),
    ] {
        if !valid {
            return Err(fail(detail));
        }
    }
    let player_sequence = u64_at(m, owner + 0x62d0)?;
    // A live startup observed different account/player sequence markers while
    // balances and clan mirrors agreed. Keep both markers in snapshot identity.
    // Only Glory needs this reconciliation barrier; unrelated resources do not
    // touch either marker. Never repair or synthesize synchronization metadata.
    let mut clans = Vec::new();
    for i in 0..60 {
        let node = account + 0x21380 + i * 32;
        let id = u32_at(m, node)?;
        if id == 0 {
            continue;
        }
        if id > 50 || clans.iter().any(|c: &Clan| c.id == id) {
            return Err(fail("unknown or duplicate clan record"));
        }
        let stored = u64_at(m, node + 0x10)?;
        let tenure = if id == current_clan {
            tenure_credit(now, joined)?
        } else {
            0
        };
        if stored.checked_add(tenure).is_none() {
            return Err(fail("contribution overflow"));
        }
        clans.push(Clan {
            id,
            node,
            stored,
            tenure,
        });
    }
    if current_clan != 0 && !clans.iter().any(|c| c.id == current_clan) {
        return Err(fail("current clan record missing"));
    }
    let mut ingot_owned = false;
    for container in crate::character::LiveItemContainer::ALL {
        let array = crate::character::read_item_array(m, owner, container)
            .map_err(|_| fail("item ownership layout unavailable"))?;
        ingot_owned |= array
            .chunks_exact(crate::character::ITEM_RECORD_BYTES)
            .any(|r| r[..2] == [0x4d, 0xdf]);
    }
    Ok(Snapshot {
        pid: m.process().pid,
        birth: m.process().creation_filetime,
        owner,
        wrapper,
        account,
        ingot,
        fragments,
        glory,
        current_clan,
        joined,
        sequence,
        player_sequence,
        clans,
        ingot_owned,
    })
}
pub fn snapshot_at(m: &dyn InventoryMemory, now: u64) -> Result<Snapshot, RuntimeError> {
    let first = once(m, now)?;
    let second = once(m, now)?;
    if first != second || m.recheck_creation_filetime()? != first.birth {
        return Err(fail("state changed while reading"));
    }
    Ok(first)
}
pub fn snapshot(m: &dyn InventoryMemory) -> Result<Snapshot, RuntimeError> {
    snapshot_at(m, clock(m)?)
}
pub fn apply(
    m: &dyn InventoryMemory,
    writer: &mut dyn TargetProcess,
    before: &Snapshot,
    field: &str,
    clan: u32,
    value: u64,
) -> LiveEditOutcome {
    apply_read(m, writer, before, field, clan, value, &|| snapshot(m))
}
#[cfg(any(test, feature = "test-fake"))]
pub fn apply_at(
    m: &dyn InventoryMemory,
    writer: &mut dyn TargetProcess,
    before: &Snapshot,
    field: &str,
    clan: u32,
    value: u64,
    now: u64,
) -> LiveEditOutcome {
    apply_read(m, writer, before, field, clan, value, &|| {
        snapshot_at(m, now)
    })
}
fn apply_read(
    m: &dyn InventoryMemory,
    writer: &mut dyn TargetProcess,
    before: &Snapshot,
    field: &str,
    clan: u32,
    value: u64,
    read: &dyn Fn() -> Result<Snapshot, RuntimeError>,
) -> LiveEditOutcome {
    let prepared = (|| {
        let (next, patches) = before.plan(field, clan, value)?;
        if writer.pid() != before.pid
            || writer.creation_filetime()? != Some(before.birth)
            || writer.exited()?
            || read()? != *before
        {
            return Err(fail("reviewed process or snapshot changed"));
        }
        Ok((next, patches))
    })();
    let (next, patches) = match prepared {
        Ok(p) => p,
        Err(e) => return LiveEditOutcome::Rejected(e.to_string()),
    };
    for patch in patches {
        // Recheck each target before dispatch; after any attempted write, every
        // failure is uncertain, including a partial multi-field Glory update.
        if bytes(m, patch.address, patch.before.len()).ok().as_ref() != Some(&patch.before) {
            return LiveEditOutcome::Uncertain("Resource target changed during write".into());
        }
        if let Err(e) = writer.write(patch.address, &patch.after) {
            return LiveEditOutcome::Uncertain(e.to_string());
        }
    }
    match read() {
        Ok(actual) if actual == next => LiveEditOutcome::Verified,
        _ => {
            LiveEditOutcome::Uncertain("Resource readback differs; check recorded operation".into())
        }
    }
}
#[cfg(any(test, feature = "test-fake"))]
#[path = "player_resources/tests.rs"]
pub mod test_fixture;
