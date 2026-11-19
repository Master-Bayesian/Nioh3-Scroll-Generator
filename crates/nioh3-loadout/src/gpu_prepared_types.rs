//! ABI output records shared by the prepared CUDA adapter and portable tests.

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct PreparedDamage {
    pub damage: crate::damage::DamageWords,
    pub bail: u32,
    pub(super) bail_rva: u32,
    pub(super) bail_address: u64,
}
const _: () = assert!(std::mem::size_of::<PreparedDamage>() == 32);
