//! Integer and binary32 RNG operations used by the PC v2.01 enemy-state port.
//!
//! This is a direct port of `nioh3_scroll_editor/enemy_state_rng.py`. The LCG
//! and the MT streams are separate: local/MT draws never advance the parent.
//! There is no game process, live-memory library, numeric-array crate, or
//! random-number crate involved.
//!
//! Diagnostic traces are out of contract. The Python reference records
//! `LcgStream.events` as evidence bookkeeping; no returned value depends on
//! them, so this port does not model them.

/// 32-bit truncation mask shared by the parent LCG.
pub const MASK32: u32 = 0xFFFF_FFFF;
/// Multiplier of the parent LCG (`state = state * A + 1 mod 2**32`).
pub const A: u32 = 69069;
/// Modular inverse of [`A`] modulo 2**32.
pub const A_INV: u32 = 0xA5E2_A705;
/// Size in bytes of the native configuration row read by
/// [`threshold_from_config`].
pub const CONFIG_ROW_BYTES: usize = 0x20;

/// Failure surface shared with the Python reference implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DomainError {
    /// `threshold_from_config` received a row that is not 0x20 bytes long.
    RowLength(usize),
}

/// Round to binary32 exactly as `struct.pack('<f', ...)` does, including the
/// overflow-to-infinity path that Python reports as `OverflowError`.
///
/// Rust float-to-float casts round to nearest, ties to even, and saturate to
/// infinity on overflow, which matches the reference for every input.
pub fn f32_of(value: f64) -> f32 {
    value as f32
}

/// Truncate toward zero under the Python reference contract.
///
/// The reference rejects anything that is not finite or not inside
/// `[-2**31, 2**31)` *before* truncating, so NaN, infinities, and out-of-range
/// values all yield `i32::MIN`, including fractional values just below the
/// lower bound. This mirrors `enemy_state_rng.cvtt_i32`; it is not a general
/// claim of hardware conversion equivalence.
///
/// The Rust `as` cast is deliberately not used for the out-of-range path
/// because it saturates instead of truncating in that region.
pub fn cvtt_i32(value: f64) -> i32 {
    if !value.is_finite() || !(-2147483648.0..2147483648.0).contains(&value) {
        i32::MIN
    } else {
        value.trunc() as i32
    }
}

/// Native lottery draw for a 16-bit ticket: two binary32 roundings, one
/// truncation, capped at 9999.
pub fn lottery_10000(high16: u16) -> i32 {
    let scaled = f32_of(f64::from(high16) / 65536.0) as f64;
    cvtt_i32(f32_of(scaled * 10000.0) as f64).min(9999)
}

/// Threshold derived from a captured 0x20-byte configuration row.
///
/// `None` models the observed native lookup failure, which is not an absent
/// capture, and returns 0.
pub fn threshold_from_config(row: Option<&[u8]>) -> Result<i32, DomainError> {
    let Some(row) = row else { return Ok(0) };
    if row.len() != CONFIG_ROW_BYTES {
        return Err(DomainError::RowLength(row.len()));
    }
    let base = i32::from_le_bytes([row[0x10], row[0x11], row[0x12], row[0x13]]);
    let mult = f32::from_le_bytes([row[0x18], row[0x19], row[0x1A], row[0x1B]]);
    if mult == 1.0 {
        return Ok(base);
    }
    let scaled = f32_of(f64::from(base)) as f64;
    Ok(cvtt_i32(f32_of(scaled * f64::from(mult)) as f64))
}

/// Parent LCG stream. Each draw advances once and exposes the high 16 bits.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LcgStream {
    state: u32,
    draws: u64,
}

impl LcgStream {
    /// Start a stream from a seed.
    ///
    /// The seed is already a `u32`, so the reference's `& 0xFFFFFFFF` step is
    /// redundant here and is dropped.
    pub fn new(seed: u32) -> Self {
        Self {
            state: seed,
            draws: 0,
        }
    }

    /// Resume a stream mid-sequence from an explicit state and draw count.
    pub fn with_progress(state: u32, draws: u64) -> Self {
        Self { state, draws }
    }

    /// Current 32-bit state.
    pub fn state(&self) -> u32 {
        self.state
    }

    /// Number of draws consumed so far.
    pub fn draws(&self) -> u64 {
        self.draws
    }

    /// Advance once and return the high 16 bits of the new state.
    pub fn u16(&mut self) -> u16 {
        self.state = self.state.wrapping_mul(A).wrapping_add(1);
        self.draws += 1;
        (self.state >> 16) as u16
    }
}

/// Words in the MT19937 state.
pub const MT_N: usize = 624;
/// Twist offset used by the native refill.
pub const MT_M: usize = 397;
/// Twist matrix element.
pub const MT_MATRIX_A: u32 = 0x9908_B0DF;
/// Upper bit mask applied to the current word.
pub const MT_UPPER_MASK: u32 = 0x8000_0000;
/// Lower bits taken from the next word.
pub const MT_LOWER_MASK: u32 = 0x7FFF_FFFF;

/// MT19937 with the game's two-bank recurrence.
///
/// The compact state here does not claim the game's 5000-byte in-memory ABI.
/// Only the output sequence, the draw count, and the rejection count are
/// exported, matching the reference.
#[derive(Debug, Clone)]
pub struct Mt19937 {
    words: [u32; MT_N],
    index: usize,
    draws: u64,
    rejections: u64,
}

impl Mt19937 {
    /// Seed the generator with a 32-bit value.
    pub fn new(seed: u32) -> Self {
        let mut words = [0u32; MT_N];
        words[0] = seed;
        for i in 1..MT_N {
            let x = words[i - 1];
            words[i] = 1812433253u32
                .wrapping_mul(x ^ (x >> 30))
                .wrapping_add(i as u32);
        }
        Self {
            words,
            index: MT_N,
            draws: 0,
            rejections: 0,
        }
    }

    /// Number of 32-bit outputs consumed.
    pub fn draws(&self) -> u64 {
        self.draws
    }

    /// Number of rejected outputs in inclusive-range draws.
    pub fn rejections(&self) -> u64 {
        self.rejections
    }

    /// Produce one tempered 32-bit output, twisting in place when exhausted.
    ///
    /// The in-place twist deliberately reads the already-updated wraparound
    /// half, just as the native second-bank to first-bank refill does.
    pub fn u32(&mut self) -> u32 {
        if self.index == MT_N {
            for i in 0..MT_N {
                let y =
                    (self.words[i] & MT_UPPER_MASK) | (self.words[(i + 1) % MT_N] & MT_LOWER_MASK);
                self.words[i] = self.words[(i + MT_M) % MT_N]
                    ^ (y >> 1)
                    ^ if y & 1 == 1 { MT_MATRIX_A } else { 0 };
            }
            self.index = 0;
        }
        let mut y = self.words[self.index];
        self.index += 1;
        self.draws += 1;
        y ^= y >> 11;
        y ^= (y << 7) & 0x9D2C_5680;
        y ^= (y << 15) & 0xEFC6_0000;
        y ^ (y >> 18)
    }

    /// Draw a value in `0..=upper`.
    ///
    /// A singleton range consumes no output. Any other range uses rejection
    /// sampling with a truncated 64-bit limit so the distribution matches the
    /// native draw.
    pub fn inclusive(&mut self, upper: u32) -> u32 {
        if upper == 0 {
            return 0;
        }
        if upper == MASK32 {
            return self.u32();
        }
        let size = u64::from(upper) + 1;
        let limit = (0x1_0000_0000u64 / size) * size;
        loop {
            let x = u64::from(self.u32());
            if x < limit {
                return (x % size) as u32;
            }
            self.rejections += 1;
        }
    }
}

/// Forward Fisher-Yates using the native generator, not a reverse iteration.
///
/// Generic over the element type so callers can shuffle descriptor IDs or any
/// other ordering key without forcing a `u32` shape.
///
/// The native draw bound is a `u32`, so the slice length is the only value that
/// could overflow that conversion.
pub fn native_shuffle<T>(values: &mut [T], mt: &mut Mt19937) {
    assert!(
        values.len().saturating_sub(1) <= u32::MAX as usize,
        "native_shuffle requires at most 2**32 elements, got {}",
        values.len()
    );
    for i in 1..values.len() {
        let bound = u32::try_from(i).expect("slice length checked above");
        let j = mt.inclusive(bound) as usize;
        if j != i {
            values.swap(i, j);
        }
    }
}

/// Affine coefficients of the parent LCG after `draw` steps: `(a, c)`.
pub fn affine(draw: u64) -> (u32, u32) {
    let (mut a, mut c, mut ba, mut bc) = (1u32, 0u32, A, 1u32);
    let mut exponent = draw;
    while exponent != 0 {
        if exponent & 1 == 1 {
            a = ba.wrapping_mul(a);
            c = ba.wrapping_mul(c).wrapping_add(bc);
        }
        let next_ba = ba.wrapping_mul(ba);
        bc = ba.wrapping_mul(bc).wrapping_add(bc);
        ba = next_ba;
        exponent >>= 1;
    }
    (a, c)
}

/// Parent LCG state after `draw` steps from `seed`.
pub fn state_after(seed: u32, draw: u64) -> u32 {
    let (a, c) = affine(draw);
    a.wrapping_mul(seed).wrapping_add(c)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Test-local row builder: the public API only reads captured rows.
    fn config_row(base: i32, mult_bits: u32) -> [u8; CONFIG_ROW_BYTES] {
        let mut row = [0u8; CONFIG_ROW_BYTES];
        row[0x10..0x14].copy_from_slice(&base.to_le_bytes());
        row[0x18..0x1C].copy_from_slice(&mult_bits.to_le_bytes());
        row
    }

    #[test]
    fn lcg_matches_single_step_arithmetic() {
        assert_eq!(state_after(0, 1), 1);
        assert_eq!(state_after(0, 2), A + 1);
        assert_eq!(state_after(0xDEAD_BEEF, 0), 0xDEAD_BEEF);
    }

    #[test]
    fn lcg_stream_exposes_high_16_bits() {
        let mut stream = LcgStream::new(0);
        assert_eq!(stream.u16(), 0);
        assert_eq!(stream.state(), 1);
        assert_eq!(stream.draws(), 1);
        let mut resumed = LcgStream::with_progress(stream.state(), stream.draws());
        assert_eq!(resumed.u16(), 1);
        assert_eq!(resumed.state(), 69070);
        assert_eq!(resumed.draws(), 2);
    }

    #[test]
    fn cvtt_saturates_to_int_min_outside_range() {
        assert_eq!(cvtt_i32(f64::NAN), i32::MIN);
        assert_eq!(cvtt_i32(f64::INFINITY), i32::MIN);
        assert_eq!(cvtt_i32(f64::NEG_INFINITY), i32::MIN);
        assert_eq!(cvtt_i32(2147483648.0), i32::MIN);
        assert_eq!(cvtt_i32(-2147483649.0), i32::MIN);
        assert_eq!(cvtt_i32(-1.75), -1);
        assert_eq!(cvtt_i32(2147483647.5), 2147483647);
    }

    #[test]
    fn lottery_anchor_and_cap_match_the_reference() {
        // Known pair from nioh3_scroll_editor.enemy_state_rng.lottery_10000.
        assert_eq!(lottery_10000(29039), 4431);
        assert_eq!(lottery_10000(65535), 9999);
        assert_eq!(lottery_10000(0), 0);
    }

    #[test]
    fn mt19937_matches_the_standard_seed_5489_stream() {
        let mut mt = Mt19937::new(5489);
        assert_eq!(mt.u32(), 3499211612);
        assert_eq!(mt.u32(), 581869302);
        assert_eq!(mt.u32(), 3890346734);
        assert_eq!(mt.draws(), 3);
    }

    #[test]
    fn singleton_range_consumes_no_output() {
        let mut mt = Mt19937::new(1);
        assert_eq!(mt.inclusive(0), 0);
        assert_eq!(mt.draws(), 0);
    }

    #[test]
    fn forward_shuffle_is_not_reverse_iteration() {
        let mut values = vec![0, 1, 2, 3, 4];
        native_shuffle(&mut values, &mut Mt19937::new(7));
        // Reference output from nioh3_scroll_editor.enemy_state_rng for the
        // same seed and values.
        assert_eq!(values, vec![0, 4, 1, 2, 3]);
    }

    #[test]
    fn threshold_requires_the_exact_row_length() {
        assert_eq!(threshold_from_config(None).unwrap(), 0);
        assert_eq!(
            threshold_from_config(Some(&[0u8; 0x1F])),
            Err(DomainError::RowLength(0x1F))
        );
        assert_eq!(
            threshold_from_config(Some(&[0u8; 0x21])),
            Err(DomainError::RowLength(0x21))
        );
        assert_eq!(
            threshold_from_config(Some(&config_row(-7, 1.0f32.to_bits()))).unwrap(),
            -7
        );
    }
}
