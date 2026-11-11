//! PC v2.02 title-prestige budgets. Only one category's earned-budget u32 is
//! writable; title flags, fame, ranks, bonus levels and allocations are read-only.
//! Evidence: deliverables/prestige-research-20261014 in the research checkout.
use crate::character::{locate_player, LiveEditOutcome};
use crate::error::RuntimeError;
use crate::inventory::InventoryMemory;
use crate::mutation::memory::TargetProcess;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub const POINTER_RVA: u64 = 0x4751860;
pub const MAX_BUDGET: u32 = 2000;
pub const CATEGORIES: [u32; 9] = [50152, 19338, 41583, 29207, 60307, 1486, 18917, 50326, 41813];
// (bonus key, category index). Order is native row order.
const BONUSES: [(u16, usize); 36] = [
    (41768, 0),
    (34270, 0),
    (45068, 0),
    (29920, 0),
    (36156, 1),
    (18444, 1),
    (46426, 1),
    (10767, 1),
    (8826, 2),
    (17963, 2),
    (15721, 2),
    (37851, 2),
    (31847, 3),
    (8200, 3),
    (31922, 3),
    (8772, 3),
    (51300, 4),
    (18340, 4),
    (65222, 4),
    (50242, 4),
    (56188, 5),
    (57434, 5),
    (1302, 5),
    (46192, 5),
    (54786, 6),
    (48133, 6),
    (15587, 6),
    (64473, 6),
    (55302, 7),
    (19272, 7),
    (47259, 7),
    (53643, 7),
    (6709, 8),
    (51436, 8),
    (30526, 8),
    (10519, 8),
];
pub const SITES: &[(u64, &[u8])] = &[
    (0x7049A2, &[0x48, 0x8B, 0x0D, 0xB7, 0xCE, 0x04, 0x04]),
    (0x7049F0, &[0x49, 0x8D, 0x8B, 0x68, 0x5E, 0, 0]),
    (0x703B25, &[0x48, 0x8B, 0x08, 0x01, 0x59, 0x1C]),
    (0x5534B3, &[0x8B, 0x40, 0x18, 0xC3]),
    (0x55343B, &[0x8B, 0x40, 0x1C, 0xC3]),
];

fn invalid(detail: &str) -> RuntimeError {
    RuntimeError::InventoryChain {
        detail: format!("prestige: {detail}"),
    }
}
fn bytes(m: &dyn InventoryMemory, at: u64, n: usize) -> Result<Vec<u8>, RuntimeError> {
    let result = m.read(at, n)?;
    if result.len() != n {
        return Err(invalid("short read"));
    }
    Ok(result)
}
fn word(m: &dyn InventoryMemory, at: u64) -> Result<u32, RuntimeError> {
    Ok(u32::from_le_bytes(
        bytes(m, at, 4)?
            .try_into()
            .map_err(|_| invalid("short u32"))?,
    ))
}
fn ptr(m: &dyn InventoryMemory, at: u64) -> Result<u64, RuntimeError> {
    let p = u64::from_le_bytes(
        bytes(m, at, 8)?
            .try_into()
            .map_err(|_| invalid("short pointer"))?,
    );
    if !(0x10000..=0x7fff_ffff_fff8).contains(&p) || p % 8 != 0 {
        return Err(invalid("invalid pointer"));
    }
    Ok(p)
}
#[derive(Debug, Clone, PartialEq, Eq)]
struct Map {
    address: u64,
    sentinel: u64,
    buckets: u64,
    count: u64,
    mask: u64,
}
impl Map {
    fn read(m: &dyn InventoryMemory, address: u64, max: u64) -> Result<Self, RuntimeError> {
        let sentinel = ptr(m, address + 8)?;
        let buckets = ptr(m, address + 0x18)?;
        let count = u64::from_le_bytes(
            bytes(m, address + 0x10, 8)?
                .try_into()
                .map_err(|_| invalid("count"))?,
        );
        let mask = u64::from_le_bytes(
            bytes(m, address + 0x30, 8)?
                .try_into()
                .map_err(|_| invalid("mask"))?,
        );
        if count > max || mask > 4095 || !(mask + 1).is_power_of_two() {
            return Err(invalid("map exceeds bounds"));
        }
        Ok(Self {
            address,
            sentinel,
            buckets,
            count,
            mask,
        })
    }
    fn find(&self, m: &dyn InventoryMemory, key: &[u8]) -> Result<Option<u64>, RuntimeError> {
        let hash = key.iter().fold(0xcbf29ce484222325u64, |h, b| {
            (h ^ u64::from(*b)).wrapping_mul(0x100000001b3)
        });
        let bucket = self.buckets + (hash & self.mask) * 16;
        let first = ptr(m, bucket)?;
        let mut node = ptr(m, bucket + 8)?;
        if node == self.sentinel {
            if first != self.sentinel {
                return Err(invalid("inconsistent empty bucket"));
            }
            return Ok(None);
        }
        let mut seen = BTreeSet::new();
        for _ in 0..self.count {
            if node == self.sentinel || !seen.insert(node) {
                return Err(invalid("invalid bucket chain"));
            }
            if bytes(m, node + 0x10, key.len())? == key {
                return Ok(Some(node));
            }
            if node == first {
                return Ok(None);
            }
            node = ptr(m, node + 8)?;
        }
        Err(invalid("bucket traversal exceeded node count"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Category {
    pub id: u32,
    pub rank: u32,
    pub fame: u32,
    pub earned: u32,
    pub spent: u32,
    node: Option<u64>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Snapshot {
    pub pid: u32,
    pub birth: u64,
    owner: u64,
    wrapper: u64,
    category_map: Map,
    bonus_map: Map,
    pub categories: Vec<Category>,
    bonuses: Vec<(u16, Option<u64>, u32, u32)>,
}
impl Snapshot {
    pub fn id(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(format!("nioh3-prestige/v1:{self:?}"))
        )
    }
    pub fn json(&self) -> Value {
        json!({"prestige": {
            "process_id":self.pid,"process_birth":self.birth.to_string(),"snapshot_id":self.id(),
            "max_budget":MAX_BUDGET,"experimental":true,
            "categories":self.categories.iter().map(|r|json!({
                "category_id":r.id,"rank":r.rank,"fame":r.fame,"earned":r.earned,
                "spent":r.spent,"available":r.earned-r.spent,"editable":r.node.is_some()
            })).collect::<Vec<_>>()
        }})
    }
    pub fn replacement(&self, category: u32, available: u32) -> Result<Self, RuntimeError> {
        let mut next = self.clone();
        let row = next
            .categories
            .iter_mut()
            .find(|r| r.id == category)
            .ok_or_else(|| invalid("unknown category"))?;
        if row.node.is_none() {
            return Err(invalid("category has no existing point record"));
        }
        let earned = row
            .spent
            .checked_add(available)
            .filter(|v| *v <= MAX_BUDGET)
            .ok_or_else(|| invalid("earned budget exceeds 2000"))?;
        if earned == row.earned {
            return Err(invalid("nothing to change"));
        }
        row.earned = earned;
        Ok(next)
    }
}

fn once(m: &dyn InventoryMemory) -> Result<Snapshot, RuntimeError> {
    for (rva, expected) in SITES {
        if bytes(m, m.module_base() + rva, expected.len())? != *expected {
            return Err(RuntimeError::SignatureMismatch {
                site: "prestige_budget".into(),
                rva: *rva,
            });
        }
    }
    let wrapper = ptr(m, m.module_base() + POINTER_RVA)?;
    let owner = ptr(m, wrapper)?;
    // Independent known player/inventory roots must identify the same object.
    if owner != locate_player(m)? {
        return Err(invalid("prestige owner is not the loaded player"));
    }
    let category_map = Map::read(m, owner + 0x5e68, 9)?;
    let bonus_map = Map::read(m, owner + 0x5ea8, 36)?;
    let mut categories = Vec::new();
    let mut nodes = BTreeSet::new();
    for id in CATEGORIES {
        let node = category_map.find(m, &id.to_le_bytes())?;
        let (rank, fame, earned) = if let Some(p) = node {
            if !nodes.insert(p) {
                return Err(invalid("duplicate category node"));
            }
            (word(m, p + 0x14)?, word(m, p + 0x18)?, word(m, p + 0x1c)?)
        } else {
            (0, 0, 0)
        };
        if rank > 22 || fame > 1100 {
            return Err(invalid("unknown fame state"));
        }
        categories.push(Category {
            id,
            rank,
            fame,
            earned,
            spent: 0,
            node,
        });
    }
    if nodes.len() as u64 != category_map.count {
        return Err(invalid("unknown category records"));
    }
    let mut bonuses = Vec::new();
    nodes.clear();
    for (key, category) in BONUSES {
        let node = bonus_map.find(m, &key.to_le_bytes())?;
        let (level, allocated) = if let Some(p) = node {
            if !nodes.insert(p) {
                return Err(invalid("duplicate bonus node"));
            }
            (word(m, p + 0x18)?, word(m, p + 0x1c)?)
        } else {
            (0, 0)
        };
        let costs: &[u32] = if category == 2 {
            &[80, 90, 100, 110, 120]
        } else {
            &[40, 42, 44, 48, 52, 58, 64, 72, 80]
        };
        let mut total = 0;
        let mut derived = 0;
        for (i, cost) in costs.iter().enumerate() {
            total += cost;
            if total <= allocated {
                derived = i as u32 + 1;
            }
        }
        if allocated > 500 || level != derived {
            return Err(invalid("inconsistent bonus allocation"));
        }
        categories[category].spent += allocated;
        bonuses.push((key, node, level, allocated));
    }
    if nodes.len() as u64 != bonus_map.count {
        return Err(invalid("unknown bonus records"));
    }
    if categories
        .iter()
        .any(|r| r.earned < r.spent || (r.node.is_none() && r.spent != 0))
    {
        return Err(invalid("point budget is below allocated points"));
    }
    let process = m.process();
    Ok(Snapshot {
        pid: process.pid,
        birth: process.creation_filetime,
        owner,
        wrapper,
        category_map,
        bonus_map,
        categories,
        bonuses,
    })
}

pub fn snapshot(m: &dyn InventoryMemory) -> Result<Snapshot, RuntimeError> {
    let first = once(m)?;
    if m.recheck_creation_filetime()? != first.birth {
        return Err(invalid("process instance changed"));
    }
    let second = once(m)?;
    if first != second || m.recheck_creation_filetime()? != first.birth {
        return Err(invalid("prestige changed while reading; reload"));
    }
    Ok(first)
}

/// The caller durably records the attempt before opening the write handle.
/// A failed attempted write is always uncertain. No rollback or replay occurs.
pub fn apply(
    m: &dyn InventoryMemory,
    writer: &mut dyn TargetProcess,
    reviewed: &Snapshot,
    category: u32,
    available: u32,
) -> LiveEditOutcome {
    let preflight = (|| {
        let next = reviewed.replacement(category, available)?;
        if writer.pid() != reviewed.pid
            || writer.creation_filetime()? != Some(reviewed.birth)
            || writer.exited()?
            || snapshot(m)? != *reviewed
        {
            return Err(invalid("reviewed process or snapshot changed"));
        }
        let row = next
            .categories
            .iter()
            .find(|r| r.id == category)
            .ok_or_else(|| invalid("unknown category"))?;
        let address = row.node.ok_or_else(|| invalid("missing category"))? + 0x1c;
        Ok((address, row.earned, next))
    })();
    let (address, earned, next) = match preflight {
        Ok(plan) => plan,
        Err(e) => return LiveEditOutcome::Rejected(e.to_string()),
    };
    if let Err(e) = writer.write(address, &earned.to_le_bytes()) {
        return LiveEditOutcome::Uncertain(e.to_string());
    }
    match snapshot(m) {
        Ok(actual) if actual == next => LiveEditOutcome::Verified,
        Ok(_) => LiveEditOutcome::Uncertain(
            "prestige readback differs; check the recorded operation".into(),
        ),
        Err(e) => LiveEditOutcome::Uncertain(e.to_string()),
    }
}

#[cfg(feature = "test-fake")]
#[path = "prestige/tests.rs"]
pub mod test_fixture;
#[cfg(test)]
pub(crate) mod tests;
