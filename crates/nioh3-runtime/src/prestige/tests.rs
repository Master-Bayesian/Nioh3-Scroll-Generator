#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::character::{EQUIPMENT_OFFSET, PLAYER_POINTER_RVA, PLAYER_VTABLE_RVA};
use crate::inventory::{InventoryProcess, CONTAINER_BIAS, GLOBAL_SLOT_RVA};
use std::cell::RefCell;
use std::collections::BTreeMap;

const BASE: u64 = 0x140000000;
const OWNER: u64 = 0x200000000;
pub struct Memory {
    data: RefCell<BTreeMap<u64, u8>>,
}
impl Memory {
    fn put(&self, at: u64, data: &[u8]) {
        self.data
            .borrow_mut()
            .extend(data.iter().enumerate().map(|(i, b)| (at + i as u64, *b)));
    }
    fn map(&self, offset: u64, rows: Vec<Vec<u8>>) {
        let map = OWNER + offset;
        let arena = OWNER + if offset == 0x5e68 { 0x100000 } else { 0x200000 };
        let sentinel = arena + 0x10000;
        let bucket = arena + 0x20000;
        self.put(map + 8, &sentinel.to_le_bytes());
        self.put(map + 0x10, &(rows.len() as u64).to_le_bytes());
        self.put(map + 0x18, &bucket.to_le_bytes());
        self.put(map + 0x30, &0u64.to_le_bytes());
        let nodes: Vec<_> = (0..rows.len())
            .map(|i| arena + 0x30000 + i as u64 * 64)
            .collect();
        self.put(
            bucket,
            &nodes.first().copied().unwrap_or(sentinel).to_le_bytes(),
        );
        self.put(
            bucket + 8,
            &nodes.last().copied().unwrap_or(sentinel).to_le_bytes(),
        );
        for (i, row) in rows.iter().enumerate() {
            self.put(
                nodes[i] + 8,
                &if i == 0 { sentinel } else { nodes[i - 1] }.to_le_bytes(),
            );
            self.put(nodes[i] + 0x10, row);
        }
    }
    pub fn fixture() -> Self {
        let m = Self {
            data: RefCell::new(BTreeMap::new()),
        };
        m.put(BASE + PLAYER_POINTER_RVA, &OWNER.to_le_bytes());
        m.put(OWNER, &(BASE + PLAYER_VTABLE_RVA).to_le_bytes());
        m.put(BASE + GLOBAL_SLOT_RVA, &0x300000000u64.to_le_bytes());
        m.put(
            0x300000000,
            &(OWNER + EQUIPMENT_OFFSET - CONTAINER_BIAS).to_le_bytes(),
        );
        m.put(BASE + POINTER_RVA, &0x300001000u64.to_le_bytes());
        m.put(0x300001000, &OWNER.to_le_bytes());
        for (rva, data) in SITES {
            m.put(BASE + rva, data);
        }
        m.map(
            0x5e68,
            CATEGORIES
                .iter()
                .map(|key| {
                    [
                        key.to_le_bytes(),
                        10u32.to_le_bytes(),
                        305u32.to_le_bytes(),
                        500u32.to_le_bytes(),
                    ]
                    .concat()
                })
                .collect(),
        );
        m.map(
            0x5ea8,
            BONUSES
                .iter()
                .map(|(key, cat)| {
                    let mut d = vec![0; 16];
                    d[..2].copy_from_slice(&key.to_le_bytes());
                    d[8..12].copy_from_slice(&1u32.to_le_bytes());
                    d[12..16].copy_from_slice(&if *cat == 2 { 80u32 } else { 40u32 }.to_le_bytes());
                    d
                })
                .collect(),
        );
        m
    }
}
impl InventoryMemory for Memory {
    fn module_base(&self) -> u64 {
        BASE
    }
    fn process(&self) -> InventoryProcess {
        InventoryProcess {
            pid: 42,
            creation_filetime: 77,
        }
    }
    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
        Ok(77)
    }
    fn read(&self, at: u64, n: usize) -> Result<Vec<u8>, RuntimeError> {
        let d = self.data.borrow();
        (0..n)
            .map(|i| {
                d.get(&(at + i as u64))
                    .copied()
                    .ok_or_else(|| invalid("unmapped test read"))
            })
            .collect()
    }
}
#[cfg(test)]
struct Writer<'a> {
    m: &'a Memory,
    writes: Vec<(u64, Vec<u8>)>,
    birth: u64,
    fail: bool,
    drop_write: bool,
}
#[cfg(test)]
impl TargetProcess for Writer<'_> {
    fn pid(&self) -> u32 {
        42
    }
    fn read(&mut self, at: u64, n: usize) -> Result<Vec<u8>, RuntimeError> {
        self.m.read(at, n)
    }
    fn write(&mut self, at: u64, d: &[u8]) -> Result<(), RuntimeError> {
        self.writes.push((at, d.to_vec()));
        if self.fail {
            self.m.put(at, &d[..1]);
            return Err(invalid("partial write"));
        }
        if !self.drop_write {
            self.m.put(at, d);
        }
        Ok(())
    }
    fn write_code(&mut self, _: u64, _: &[u8]) -> Result<(), RuntimeError> {
        panic!("code writes forbidden")
    }
    fn allocate_executable_near(&mut self, _: u64, _: usize) -> Result<u64, RuntimeError> {
        panic!("allocation forbidden")
    }
    fn free_allocation(&mut self, _: u64) -> Result<(), RuntimeError> {
        panic!("allocation forbidden")
    }
    fn exited(&mut self) -> Result<bool, RuntimeError> {
        Ok(false)
    }
    fn creation_filetime(&mut self) -> Result<Option<u64>, RuntimeError> {
        Ok(Some(self.birth))
    }
    fn close(&mut self) {}
}
#[cfg(test)]
fn writer(m: &Memory) -> Writer<'_> {
    Writer {
        m,
        writes: vec![],
        birth: 77,
        fail: false,
        drop_write: false,
    }
}

#[test]
fn snapshots_derive_independent_available_budgets_and_colliding_keys() {
    let m = Memory::fixture();
    let s = snapshot(&m).unwrap();
    assert_eq!(s.categories.len(), 9);
    assert_eq!(s.categories[0].spent, 160);
    assert_eq!(s.categories[2].spent, 320);
    assert_eq!(s.json()["prestige"]["categories"][0]["available"], 340);
    assert_eq!(s.json()["prestige"]["categories"][2]["available"], 180);
}
#[test]
fn every_category_writes_exactly_its_budget_u32_and_preserves_all_other_bytes() {
    for key in CATEGORIES {
        let m = Memory::fixture();
        let before = snapshot(&m).unwrap();
        let bytes_before = m.data.borrow().clone();
        let mut w = writer(&m);
        assert_eq!(
            apply(&m, &mut w, &before, key, 123),
            LiveEditOutcome::Verified
        );
        let at = before
            .categories
            .iter()
            .find(|r| r.id == key)
            .unwrap()
            .node
            .unwrap()
            + 0x1c;
        assert_eq!(w.writes.len(), 1);
        assert_eq!(w.writes[0].0, at);
        assert_eq!(w.writes[0].1.len(), 4);
        let mut after_bytes = m.data.borrow().clone();
        for i in 0..4 {
            after_bytes.insert(at + i, bytes_before[&(at + i)]);
        }
        assert_eq!(after_bytes, bytes_before);
    }
}
#[test]
fn stale_budget_or_allocation_rejects_before_writing() {
    for allocation in [false, true] {
        let m = Memory::fixture();
        let before = snapshot(&m).unwrap();
        let at = if allocation {
            before.bonuses[0].1.unwrap() + 0x1c
        } else {
            before.categories[0].node.unwrap() + 0x1c
        };
        let original = word(&m, at).unwrap();
        m.put(at, &(original + 1).to_le_bytes());
        let mut w = writer(&m);
        assert!(matches!(
            apply(&m, &mut w, &before, CATEGORIES[0], 123),
            LiveEditOutcome::Rejected(_)
        ));
        assert!(w.writes.is_empty());
    }
}
#[test]
fn recycled_write_handle_is_refused() {
    let m = Memory::fixture();
    let s = snapshot(&m).unwrap();
    let mut w = writer(&m);
    w.birth = 78;
    assert!(matches!(
        apply(&m, &mut w, &s, CATEGORIES[0], 123),
        LiveEditOutcome::Rejected(_)
    ));
    assert!(w.writes.is_empty());
}
#[test]
fn invalid_budget_and_unknown_category_never_write() {
    let m = Memory::fixture();
    let s = snapshot(&m).unwrap();
    for (key, value) in [
        (1, 100),
        (CATEGORIES[0], 2000),
        (CATEGORIES[0], u32::MAX),
        (CATEGORIES[0], 340),
    ] {
        let mut w = writer(&m);
        assert!(matches!(
            apply(&m, &mut w, &s, key, value),
            LiveEditOutcome::Rejected(_)
        ));
        assert!(w.writes.is_empty());
    }
}
#[test]
fn zero_available_and_exact_full_allocation_capacity_are_supported() {
    for available in [0, 1840] {
        let m = Memory::fixture();
        let s = snapshot(&m).unwrap();
        let mut w = writer(&m);
        assert_eq!(
            apply(&m, &mut w, &s, CATEGORIES[0], available),
            LiveEditOutcome::Verified
        );
    }
}
#[test]
fn partial_or_dropped_writes_are_uncertain_and_never_rolled_back_or_retried() {
    for fail in [true, false] {
        let m = Memory::fixture();
        let s = snapshot(&m).unwrap();
        let mut w = writer(&m);
        w.fail = fail;
        w.drop_write = !fail;
        assert!(matches!(
            apply(&m, &mut w, &s, CATEGORIES[0], 123),
            LiveEditOutcome::Uncertain(_)
        ));
        assert_eq!(w.writes.len(), 1);
    }
}
#[test]
fn corrupt_layouts_signatures_cycles_and_unknown_records_fail_closed() {
    for fault in 0..7 {
        let m = Memory::fixture();
        let s = snapshot(&m).unwrap();
        match fault {
            0 => m.put(BASE + SITES[0].0, &[0]),
            1 => m.put(0x300001000, &0x200100000u64.to_le_bytes()),
            2 => m.put(OWNER + 0x5e68 + 0x30, &4096u64.to_le_bytes()),
            3 => m.put(OWNER + 0x5e68 + 0x10, &10u64.to_le_bytes()),
            4 => m.put(s.bonuses[0].1.unwrap() + 0x18, &9u32.to_le_bytes()),
            5 => m.put(s.categories[0].node.unwrap() + 0x10, &1u32.to_le_bytes()),
            _ => {
                let tail = s.categories[8].node.unwrap();
                m.put(tail + 8, &tail.to_le_bytes());
            }
        }
        assert!(snapshot(&m).is_err(), "fault {fault}");
    }
}
#[test]
fn empty_maps_read_as_zero_but_missing_category_is_not_created() {
    let m = Memory::fixture();
    m.map(0x5e68, vec![]);
    m.map(0x5ea8, vec![]);
    let s = snapshot(&m).unwrap();
    let mut w = writer(&m);
    assert!(s
        .categories
        .iter()
        .all(|r| r.node.is_none() && r.spent == 0 && r.earned == 0));
    assert!(matches!(
        apply(&m, &mut w, &s, CATEGORIES[0], 100),
        LiveEditOutcome::Rejected(_)
    ));
    assert!(w.writes.is_empty());
}
