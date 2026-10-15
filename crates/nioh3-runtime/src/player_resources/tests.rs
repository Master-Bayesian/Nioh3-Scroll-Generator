#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
use super::*;
use crate::character::{LiveItemContainer, ITEM_RECORD_BYTES};
use crate::inventory::InventoryProcess;
#[cfg(feature = "test-fake")]
use crate::prestige::test_fixture::Memory as InnerMemory;
#[cfg(all(test, not(feature = "test-fake")))]
use crate::prestige::tests::Memory as InnerMemory;
use std::cell::RefCell;
use std::collections::BTreeMap;
pub const BASE: u64 = 0x140000000;
pub const OWNER: u64 = 0x200000000;
pub const ACCOUNT: u64 = 0x400000000;
pub const NOW: u64 = 1_800_000_000;
pub struct Memory {
    inner: InnerMemory,
    pub edits: RefCell<BTreeMap<u64, u8>>,
    pub ingot_owned: bool,
}
impl Memory {
    pub fn put(&self, at: u64, data: &[u8]) {
        self.edits
            .borrow_mut()
            .extend(data.iter().enumerate().map(|(i, v)| (at + i as u64, *v)));
    }
    pub fn fixture() -> Self {
        let m = Self {
            inner: InnerMemory::fixture(),
            edits: RefCell::new(BTreeMap::new()),
            ingot_owned: true,
        };
        for (rva, bytes) in SITES {
            m.put(BASE + rva, bytes);
        }
        for (i, v) in THRESHOLDS.iter().enumerate() {
            m.put(BASE + 0x3bdcb30 + i as u64 * 8, &v.to_le_bytes());
        }
        m.put(BASE + ACCOUNT_RVA, &ACCOUNT.to_le_bytes());
        m.put(OWNER + 0x30, &1234567890123u64.to_le_bytes());
        m.put(OWNER + 0x20, &12345u32.to_le_bytes());
        m.put(OWNER + 0x62dc, &12345u32.to_le_bytes());
        m.put(ACCOUNT + 0x21274, &12345u32.to_le_bytes());
        m.put(OWNER + 0x62e0, &7u32.to_le_bytes());
        m.put(ACCOUNT + 0x21220, &7u32.to_le_bytes());
        m.put(OWNER + 0x62d0, &88u64.to_le_bytes());
        m.put(ACCOUNT + 0x21268, &88u64.to_le_bytes());
        m.put(ACCOUNT + 0x21280, &(NOW - 86400).to_le_bytes());
        m.put(ACCOUNT + 0x21380, &vec![0; 60 * 32]);
        for (index, id) in [3u32, 7, 50].iter().enumerate() {
            let at = ACCOUNT + 0x21380 + index as u64 * 32;
            m.put(at, &id.to_le_bytes());
            m.put(at + 0x10, &15915u64.to_le_bytes());
        }
        m
    }
}
impl InventoryMemory for Memory {
    fn module_base(&self) -> u64 {
        BASE
    }
    fn process(&self) -> InventoryProcess {
        self.inner.process()
    }
    fn recheck_creation_filetime(&self) -> Result<u64, RuntimeError> {
        Ok(77)
    }
    fn read(&self, at: u64, n: usize) -> Result<Vec<u8>, RuntimeError> {
        let mut original = None;
        for container in LiveItemContainer::ALL {
            let start = OWNER + container.offset();
            let size = container.slots() * ITEM_RECORD_BYTES;
            if at == start && n == size {
                let mut data = vec![0; size];
                if self.ingot_owned && container == LiveItemContainer::Held {
                    data[..2].copy_from_slice(&0xdf4du16.to_le_bytes());
                    data[0x18..0x1c].copy_from_slice(&0x200000u32.to_le_bytes());
                }
                original = Some(data);
            }
            if at == start - 8 && n == 8 {
                original = Some(
                    if container == LiveItemContainer::Held {
                        2500u64
                    } else {
                        4000u64
                    }
                    .to_le_bytes()
                    .to_vec(),
                );
            }
            if at == start + size as u64 && n == 8 {
                original = Some((container.slots() as u64).to_le_bytes().to_vec());
            }
        }
        if original.is_none() {
            if self.edits.borrow().range(at..at + n as u64).count() == n {
                original = Some(vec![0; n]);
            } else {
                original = Some(self.inner.read(at, n)?);
            }
        }
        let mut data = original.unwrap();
        for (address, value) in self.edits.borrow().range(at..at + n as u64) {
            data[(*address - at) as usize] = *value;
        }
        Ok(data)
    }
}
pub struct Writer<'a> {
    pub m: &'a Memory,
    pub writes: Vec<(u64, Vec<u8>)>,
    pub birth: u64,
    pub fail_at: Option<usize>,
    pub drop_write: bool,
}
impl<'a> Writer<'a> {
    pub fn new(m: &'a Memory) -> Self {
        Self {
            m,
            writes: vec![],
            birth: 77,
            fail_at: None,
            drop_write: false,
        }
    }
}
impl TargetProcess for Writer<'_> {
    fn pid(&self) -> u32 {
        42
    }
    fn read(&mut self, a: u64, n: usize) -> Result<Vec<u8>, RuntimeError> {
        self.m.read(a, n)
    }
    fn write(&mut self, a: u64, d: &[u8]) -> Result<(), RuntimeError> {
        self.writes.push((a, d.to_vec()));
        if self.fail_at == Some(self.writes.len()) {
            self.m.put(a, &d[..1]);
            return Err(fail("injected partial write"));
        }
        if !self.drop_write {
            self.m.put(a, d);
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
#[test]
fn each_resource_changes_only_its_exact_fields_and_preserves_neighbors() {
    for (field, clan, value, widths) in [
        ("ingot", 0, INGOT_MAX, vec![8]),
        ("fragments", 0, 0, vec![4]),
        ("glory", 0, GLORY_MAX, vec![4, 4]),
        ("clan_rank", 3, 9, vec![8]),
        ("clan_rank", 7, 0, vec![8]),
    ] {
        let m = Memory::fixture();
        let before = snapshot_at(&m, NOW).unwrap();
        let original = m.edits.borrow().clone();
        let (next, patches) = before.plan(field, clan, value).unwrap();
        let mut w = Writer::new(&m);
        assert_eq!(
            apply_at(&m, &mut w, &before, field, clan, value, NOW),
            LiveEditOutcome::Verified
        );
        assert_eq!(
            w.writes.iter().map(|(_, b)| b.len()).collect::<Vec<_>>(),
            widths
        );
        assert_eq!(snapshot_at(&m, NOW).unwrap(), next);
        let mut edited = m.edits.borrow().clone();
        for patch in patches {
            for i in 0..patch.before.len() {
                edited.insert(
                    patch.address + i as u64,
                    original[&(patch.address + i as u64)],
                );
            }
        }
        assert_eq!(edited, original);
    }
}
#[test]
fn bounds_missing_ownership_and_noops_never_write() {
    let mut m = Memory::fixture();
    let s = snapshot_at(&m, NOW).unwrap();
    for (f, c, v) in [
        ("ingot", 0, INGOT_MAX + 1),
        ("fragments", 0, FRAGMENTS_MAX + 1),
        ("glory", 0, GLORY_MAX + 1),
        ("ingot", 0, u64::MAX),
        ("clan_rank", 3, 10),
        ("clan_rank", 49, 9),
        ("glory", 3, 1),
        ("unknown", 0, 1),
        ("glory", 0, 12345),
    ] {
        assert!(s.plan(f, c, v).is_err());
    }
    m.ingot_owned = false;
    assert!(snapshot_at(&m, NOW).unwrap().plan("ingot", 0, 1).is_err());
}
#[test]
fn pid_reuse_stale_values_and_partial_glory_writes_are_distinguished() {
    let m = Memory::fixture();
    let s = snapshot_at(&m, NOW).unwrap();
    let mut w = Writer::new(&m);
    w.birth = 78;
    assert!(matches!(
        apply_at(&m, &mut w, &s, "glory", 0, 99, NOW),
        LiveEditOutcome::Rejected(_)
    ));
    assert!(w.writes.is_empty());
    w.birth = 77;
    w.fail_at = Some(2);
    assert!(matches!(
        apply_at(&m, &mut w, &s, "glory", 0, 99, NOW),
        LiveEditOutcome::Uncertain(_)
    ));
    assert_eq!(w.writes.len(), 2);
    assert!(snapshot_at(&m, NOW).is_err());
    let m = Memory::fixture();
    let s = snapshot_at(&m, NOW).unwrap();
    m.put(OWNER + 0x20, &99u32.to_le_bytes());
    let mut w = Writer::new(&m);
    assert!(matches!(
        apply_at(&m, &mut w, &s, "fragments", 0, 8, NOW),
        LiveEditOutcome::Rejected(_)
    ));
    assert!(w.writes.is_empty());
}
#[test]
fn native_boundaries_tenure_steps_and_each_rank_are_exact() {
    for (i, t) in THRESHOLDS.iter().enumerate() {
        assert_eq!(rank(*t), i);
        if i > 0 {
            assert_eq!(rank(t - 1), i - 1);
        }
    }
    assert_eq!(tenure_credit(86_400_000_000, 0).unwrap(), 200);
    assert_eq!(tenure_credit(86_400_000_000 - 1, 0).unwrap(), 0);
    assert_eq!(tenure_credit(1, 2).unwrap(), 0);
    let m = Memory::fixture();
    let mut s = snapshot_at(&m, NOW).unwrap();
    s.clans[0].tenure = 3200;
    assert!(s.plan("clan_rank", 3, 1).is_err());
    assert_eq!(s.plan("clan_rank", 3, 9).unwrap().0.clans[0].stored, 56800);
}
#[test]
fn corrupted_maps_mirrors_thresholds_and_signatures_fail_closed() {
    for kind in 0..5 {
        let m = Memory::fixture();
        match kind {
            0 => m.put(OWNER + 0x62dc, &4u32.to_le_bytes()),
            1 => m.put(ACCOUNT + 0x21380 + 32, &3u32.to_le_bytes()),
            2 => m.put(BASE + SITES[0].0, &[0]),
            3 => m.put(BASE + 0x3bdcb30, &1u64.to_le_bytes()),
            _ => m.put(OWNER + 0x20, &u32::MAX.to_le_bytes()),
        }
        assert!(snapshot_at(&m, NOW).is_err());
    }
}

#[test]
fn differing_glory_markers_block_glory_but_preserve_unrelated_resource_edits() {
    let m = Memory::fixture();
    m.put(OWNER + 0x62d0, &100u64.to_le_bytes());
    let before = snapshot_at(&m, NOW).unwrap();
    assert_eq!(before.json()["player_resources"]["glory_editable"], false);
    assert!(before.plan("glory", 0, 99).is_err());
    let mut writer = Writer::new(&m);
    assert_eq!(
        apply_at(&m, &mut writer, &before, "fragments", 0, 99, NOW),
        LiveEditOutcome::Verified
    );
    assert_eq!(writer.writes.len(), 1);
    let after = snapshot_at(&m, NOW).unwrap();
    assert_eq!(after.sequence, 88);
    assert_eq!(after.player_sequence, 100);
    assert_eq!(after.glory, before.glory);
    m.put(OWNER + 0x62d0, &88u64.to_le_bytes());
    assert_eq!(
        snapshot_at(&m, NOW).unwrap().json()["player_resources"]["glory_editable"],
        true
    );
}
