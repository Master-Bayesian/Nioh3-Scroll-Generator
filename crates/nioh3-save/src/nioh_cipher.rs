//! Faithful port of the shipped `third_party/nioh_savefile_decrypt/aes.c`.
//!
//! That file is not AES. It is the tiny-AES-C control flow with two shipped
//! changes that make the cipher a different one:
//!
//! 1. both the forward and the inverse substitution use a custom Nioh table
//!    (the file's own `sbox` and `rsbox` arrays are byte-identical, so the same
//!    table is used in both directions), and
//! 2. `KeyExpansion` reverses each 4-byte word of the key as it loads the first
//!    round key, and `Rcon[0]` is `0x8D` rather than `0x01`.
//!
//! A standard AES implementation therefore cannot reproduce the shipped save
//! cipher; a probe of the vendored `AES_ECB_encrypt` on the FIPS-197 example
//! key/block returns `f9d96e1d6179a094d146667109a62059` instead of the standard
//! `69c4e0d86a7b0430d8cdb78070b4c55a`. This module is a line-for-line port of
//! the shipped primitive, pinned by that same known-answer vector, and is used
//! only to reproduce the product's own byte format. No other file in the crate
//! depends on an AES assumption.
//!
//! `state` is `uint8_t[4][4]` in the shipped source, so the block is a plain
//! row-major 16-byte matrix and no word reversal applies to it.
//!
//! ## Implementation
//!
//! The round is the shipped one, evaluated through four 256-entry tables that
//! fold `SubBytes`, `ShiftRows` and `MixColumns` into one lookup per input byte
//! (`MIX_TABLES`), with the round keys expanded once per key into
//! [`RoundCipher`] instead of once per block. That is an evaluation strategy,
//! not a different cipher: `MixColumns` is linear over GF(2), so
//! `xtime(a ^ b) == xtime(a) ^ xtime(b)` and the table entries are exactly the
//! coefficients the shipped byte-wise round applies. The byte-wise round this
//! replaced is kept in this module's tests as the differential oracle, and the
//! known-answer vectors below run against the table-driven path.
//!
//! ## Attribution
//!
//! This is a translation of third-party code that already ships with this
//! repository; it is a game-format compatibility primitive, not a new cipher
//! design and not a security boundary.
//!
//! - Source: `third_party/nioh_savefile_decrypt/aes.c`, itself "a rework of
//!   https://github.com/pawREP/Nioh-Savedata-Decryption-Tool" (`README.md`).
//! - Licence: MIT, `third_party/nioh_savefile_decrypt/LICENSE`, Copyright (c)
//!   2017 pawREP. The translation keeps the same MIT terms and adds no new
//!   licence obligations.
//! - Pinned identities (SHA-256): `bin/Nioh_Savefile_decrypt.exe`
//!   `A767E967B955082CE0FC0B44DAACFDA3903446CAA448591F5EFAB72F43F45A9A`,
//!   `third_party/nioh_savefile_decrypt/aes.c`
//!   `A2613EBDA8AC0D3830C28E0D2BA12A4F588DE4968110658E12BC90C1B69A258E`,
//!   `third_party/nioh_savefile_decrypt/CryptoState.cpp`
//!   `4132FC51DFF6BC627E8AA9F6D86FD4DF87EE1279F294EAEEC0B599976E7885B1`.
//! - The format carries no message authentication code; the container's
//!   integrity rests on the decrypted magic/size and the caller's own digest
//!   checks. Nothing here pretends otherwise.

/// Number of columns in the state (AES `Nb`).
const NB: usize = 4;
/// 32-bit words in the key (AES-128 `Nk`).
const NK: usize = 4;
/// Rounds (AES-128 `Nr`).
const NR: usize = 10;
/// Expanded round-key bytes.
const KEY_EXPANSION_BYTES: usize = 176;

/// The shipped round-constant word array (`Rcon`), including its `0x8D` first entry.
const RCON: [u8; 11] = [
    0x8D, 0x01, 0x02, 0x04, 0x08, 0x10, 0x20, 0x40, 0x80, 0x1B, 0x36,
];

/// The shipped Nioh substitution table (used for both directions).
const SBOX: [u8; 256] = [
    0x1C, 0x2F, 0x03, 0x53, 0xA3, 0x01, 0x49, 0xDA, 0xA6, 0xCD, 0xE0, 0x8A, 0x19, 0xA7, 0x04, 0xD4,
    0x06, 0x1A, 0xDA, 0x49, 0x08, 0xE2, 0xF6, 0xB2, 0x9E, 0xE1, 0x22, 0x49, 0xCE, 0x7B, 0x7E, 0x5E,
    0xA0, 0x09, 0x2A, 0x63, 0xAF, 0x49, 0xCE, 0x70, 0x7B, 0x3C, 0x23, 0x80, 0xFA, 0x17, 0x47, 0xF2,
    0x62, 0x62, 0x6C, 0x59, 0x10, 0xCC, 0x29, 0x9C, 0xB5, 0x46, 0x58, 0xC7, 0x44, 0x13, 0xE7, 0x38,
    0xD5, 0xAF, 0x27, 0x83, 0xD4, 0xD5, 0xA0, 0x9E, 0xE3, 0x76, 0x3B, 0x85, 0x04, 0xD9, 0xD6, 0x98,
    0x60, 0x66, 0xD4, 0x78, 0x53, 0xEA, 0xCA, 0x0E, 0x8D, 0x56, 0x53, 0x44, 0xE2, 0xEF, 0xBD, 0xA9,
    0x9B, 0x10, 0x0A, 0xA1, 0x13, 0x93, 0xF0, 0x43, 0x0B, 0x7C, 0x39, 0x8A, 0x47, 0xDF, 0xD3, 0xC5,
    0x0E, 0x34, 0x31, 0xA6, 0xAE, 0x5A, 0xB8, 0xE7, 0xE6, 0x31, 0x43, 0xC0, 0xAA, 0x0F, 0xE0, 0x82,
    0x12, 0x4C, 0xD1, 0xDF, 0x8B, 0xA5, 0xAC, 0x70, 0xC5, 0x3D, 0x1B, 0x8E, 0x93, 0x17, 0x4D, 0x79,
    0x4E, 0xCE, 0x63, 0xC4, 0x33, 0x0E, 0x14, 0x57, 0xF0, 0xD8, 0x19, 0x5B, 0x9B, 0x61, 0x71, 0xF2,
    0x2B, 0x33, 0x7E, 0xFD, 0x2C, 0x0B, 0xB6, 0x23, 0x20, 0xB9, 0xD4, 0x91, 0x19, 0x94, 0x04, 0xA4,
    0x30, 0x13, 0x8A, 0xF1, 0xD0, 0x05, 0xEC, 0x5E, 0xAC, 0x4A, 0xD4, 0xD6, 0xA5, 0x17, 0x7F, 0xF9,
    0xE5, 0xF6, 0x00, 0x29, 0xD7, 0x93, 0x2D, 0x5E, 0x2C, 0xF1, 0x81, 0xA3, 0xB7, 0x63, 0x39, 0x57,
    0xC2, 0x33, 0x87, 0x2D, 0xA8, 0x3F, 0x02, 0xCC, 0x08, 0x67, 0x74, 0x60, 0xD8, 0xF0, 0xDA, 0x67,
    0x40, 0x64, 0x87, 0x55, 0xBB, 0x7F, 0xF2, 0x10, 0xC9, 0x03, 0x14, 0xB5, 0x80, 0x66, 0xCB, 0x91,
    0xF6, 0x1F, 0x79, 0x58, 0x88, 0xBC, 0x95, 0xC2, 0x06, 0x5F, 0xE9, 0x09, 0x32, 0xED, 0x9B, 0x85,
];

/// `xtime`.
const fn xtime(value: u8) -> u8 {
    (value << 1) ^ (((value >> 7) & 1) * 0x1B)
}

/// Expanded round keys for one key, with the shipped first-word reversal.
pub fn key_expansion(key: &[u8; 16]) -> [u8; KEY_EXPANSION_BYTES] {
    let mut round_key = [0u8; KEY_EXPANSION_BYTES];
    for index in 0..NK {
        round_key[index * 4] = key[index * 4 + 3];
        round_key[index * 4 + 1] = key[index * 4 + 2];
        round_key[index * 4 + 2] = key[index * 4 + 1];
        round_key[index * 4 + 3] = key[index * 4];
    }
    for index in NK..(NB * (NR + 1)) {
        let mut temp = [
            round_key[(index - 1) * 4],
            round_key[(index - 1) * 4 + 1],
            round_key[(index - 1) * 4 + 2],
            round_key[(index - 1) * 4 + 3],
        ];
        if index % NK == 0 {
            temp.rotate_left(1);
            for byte in temp.iter_mut() {
                *byte = SBOX[usize::from(*byte)];
            }
            temp[0] ^= RCON[index / NK];
        }
        for offset in 0..4 {
            round_key[index * 4 + offset] = round_key[(index - NK) * 4 + offset] ^ temp[offset];
        }
    }
    round_key
}

/// `MixColumns` coefficients: which input position of a row contributes which
/// multiple to each of the four output bytes.
///
/// The shipped round computes `out[c]` from the four `ShiftRows` bytes
/// `(t0, t1, t2, t3)`; expanding its `xtime` terms over GF(2) gives
/// `out[c] = XOR_p(MIX_COEFFICIENTS[p][c] * t_p)`, so row `p` holds the
/// multiples of `t_p` that reach `out[0..4]`.
const MIX_COEFFICIENTS: [[u8; 4]; 4] = [[2, 1, 1, 3], [3, 2, 1, 1], [1, 3, 2, 1], [1, 1, 3, 2]];

/// Multiply one substituted byte by a `MixColumns` coefficient.
const fn scale(coefficient: u8, value: u8) -> u8 {
    match coefficient {
        1 => value,
        2 => xtime(value),
        3 => xtime(value) ^ value,
        _ => 0,
    }
}

/// `SubBytes` + `ShiftRows` + `MixColumns` as four 256-entry tables.
///
/// Entry `[position][value]` holds the four bytes that a substituted byte at
/// `position` contributes to the four bytes of an output row, little-endian.
const fn build_mix_tables() -> [[u32; 256]; 4] {
    let mut tables = [[0u32; 256]; 4];
    let mut position = 0;
    while position < 4 {
        let mut value = 0;
        while value < 256 {
            let substituted = SBOX[value];
            tables[position][value] = u32::from_le_bytes([
                scale(MIX_COEFFICIENTS[position][0], substituted),
                scale(MIX_COEFFICIENTS[position][1], substituted),
                scale(MIX_COEFFICIENTS[position][2], substituted),
                scale(MIX_COEFFICIENTS[position][3], substituted),
            ]);
            value += 1;
        }
        position += 1;
    }
    tables
}

/// The folded `SubBytes`/`ShiftRows`/`MixColumns` tables.
const MIX_TABLES: [[u32; 256]; 4] = build_mix_tables();

/// Byte `index` of a little-endian state word.
fn state_byte(word: u32, index: u32) -> usize {
    ((word >> (8 * index)) & 0xFF) as usize
}

/// The four `ShiftRows` bytes feeding output row `row`.
///
/// The shipped `ShiftRows` moves `state[r][c]` to `state[(r + c) % 4][c]`, so
/// output row `row` takes byte `c` from the pre-round word `(row + c) % 4`.
fn shifted_bytes(state: &[u32; 4], row: usize) -> [usize; 4] {
    [
        state_byte(state[row], 0),
        state_byte(state[(row + 1) % 4], 1),
        state_byte(state[(row + 2) % 4], 2),
        state_byte(state[(row + 3) % 4], 3),
    ]
}

/// The shipped forward transform under one expanded key.
///
/// The expansion is the shipped one ([`key_expansion`]); only the round
/// evaluation is table-driven, so the ciphertext is identical byte for byte.
#[derive(Clone, Copy)]
pub struct RoundCipher {
    round_keys: [u32; NB * (NR + 1)],
}

impl RoundCipher {
    /// Expand `key` once, ready for any number of blocks.
    pub fn new(key: &[u8; 16]) -> Self {
        let expanded = key_expansion(key);
        let mut round_keys = [0u32; NB * (NR + 1)];
        for (index, word) in round_keys.iter_mut().enumerate() {
            let offset = index * 4;
            *word = u32::from_le_bytes([
                expanded[offset],
                expanded[offset + 1],
                expanded[offset + 2],
                expanded[offset + 3],
            ]);
        }
        Self { round_keys }
    }

    /// `Cipher`: the shipped forward transform of one 16-byte block.
    pub fn encrypt(&self, block: &[u8; 16]) -> [u8; 16] {
        let mut state = [
            u32::from_le_bytes([block[0], block[1], block[2], block[3]]),
            u32::from_le_bytes([block[4], block[5], block[6], block[7]]),
            u32::from_le_bytes([block[8], block[9], block[10], block[11]]),
            u32::from_le_bytes([block[12], block[13], block[14], block[15]]),
        ];
        for (word, key) in state.iter_mut().zip(&self.round_keys[..NB]) {
            *word ^= *key;
        }
        for round in 1..NR {
            let previous = state;
            let keys = &self.round_keys[round * NB..];
            for (row, word) in state.iter_mut().enumerate() {
                let [a, b, c, d] = shifted_bytes(&previous, row);
                *word = MIX_TABLES[0][a]
                    ^ MIX_TABLES[1][b]
                    ^ MIX_TABLES[2][c]
                    ^ MIX_TABLES[3][d]
                    ^ keys[row];
            }
        }
        let previous = state;
        for (row, word) in state.iter_mut().enumerate() {
            let [a, b, c, d] = shifted_bytes(&previous, row);
            // The final round drops `MixColumns` and substitutes in place.
            *word = u32::from_le_bytes([SBOX[a], SBOX[b], SBOX[c], SBOX[d]])
                ^ self.round_keys[NR * NB + row];
        }
        let mut out = [0u8; 16];
        for (row, chunk) in out.chunks_exact_mut(4).enumerate() {
            chunk.copy_from_slice(&state[row].to_le_bytes());
        }
        out
    }
}

/// `Cipher`: the shipped forward transform of one 16-byte block.
///
/// Expands the key per call, which is convenient for one-off blocks. Code that
/// runs the cipher over a region holds a [`RoundCipher`] instead.
pub fn encrypt_block(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
    RoundCipher::new(key).encrypt(block)
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::indexing_slicing,
        clippy::expect_used,
        clippy::panic,
        clippy::unwrap_used
    )]
    use super::{encrypt_block, RoundCipher};

    /// The byte-wise round this module shipped before the table-driven rewrite.
    ///
    /// It is the differential oracle for that rewrite: the two known-answer
    /// vectors below are produced by the shipped `aes.c` itself, and this
    /// implementation reproduces them, so agreement between this and
    /// [`super::RoundCipher`] on arbitrary input is agreement with the product
    /// primitive.
    fn reference_encrypt_block(key: &[u8; 16], block: &[u8; 16]) -> [u8; 16] {
        use super::{key_expansion, xtime, NB, NR, SBOX};

        fn add_round_key(state: &mut [u8; 16], round_key: &[u8; 176], round: usize) {
            for i in 0..4 {
                for j in 0..4 {
                    state[i * 4 + j] ^= round_key[round * NB * 4 + i * NB + j];
                }
            }
        }

        fn shift_rows(state: &mut [u8; 16]) {
            let source = *state;
            let mapping: [usize; 16] = [
                0, 5, 10, 15, //
                4, 9, 14, 3, //
                8, 13, 2, 7, //
                12, 1, 6, 11, //
            ];
            let mut shifted = [0u8; 16];
            for (destination, source_index) in mapping.iter().enumerate() {
                shifted[destination] = source[*source_index];
            }
            *state = shifted;
        }

        fn mix_columns(state: &mut [u8; 16]) {
            let source = *state;
            let mut mixed = [0u8; 16];
            for row in 0..4 {
                let a = source[row * 4];
                let b = source[row * 4 + 1];
                let c = source[row * 4 + 2];
                let d = source[row * 4 + 3];
                let total = a ^ b ^ c ^ d;
                mixed[row * 4] = a ^ xtime(a ^ b) ^ total;
                mixed[row * 4 + 1] = b ^ xtime(b ^ c) ^ total;
                mixed[row * 4 + 2] = c ^ xtime(c ^ d) ^ total;
                mixed[row * 4 + 3] = d ^ xtime(d ^ a) ^ total;
            }
            *state = mixed;
        }

        let round_key = key_expansion(key);
        let mut state = *block;
        add_round_key(&mut state, &round_key, 0);
        for round in 1..NR {
            for byte in state.iter_mut() {
                *byte = SBOX[usize::from(*byte)];
            }
            shift_rows(&mut state);
            mix_columns(&mut state);
            add_round_key(&mut state, &round_key, round);
        }
        for byte in state.iter_mut() {
            *byte = SBOX[usize::from(*byte)];
        }
        shift_rows(&mut state);
        add_round_key(&mut state, &round_key, NR);
        state
    }

    /// Deterministic pseudo-random bytes, so a failure is reproducible.
    struct Lcg(u64);

    impl Lcg {
        fn next_u64(&mut self) -> u64 {
            self.0 = self
                .0
                .wrapping_mul(6_364_136_223_846_793_005)
                .wrapping_add(1_442_695_040_888_963_407);
            self.0
        }

        fn fill(&mut self, bytes: &mut [u8]) {
            for byte in bytes.iter_mut() {
                *byte = (self.next_u64() >> 33) as u8;
            }
        }
    }

    /// Known-answer vector produced by the shipped `aes.c` itself.
    ///
    /// Compiling `third_party/nioh_savefile_decrypt/aes.c` with its own defines
    /// and calling `AES_ECB_encrypt` on the FIPS-197 example key/block returns
    /// this value, which is deliberately *not* the AES answer, because the
    /// shipped cipher substitutes a different S-box. Reproducing it is what
    /// proves this port matches the product rather than standard AES.
    #[test]
    fn matches_the_shipped_primitive_known_answer() {
        let key = [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D,
            0x0E, 0x0F,
        ];
        let block = [
            0x00, 0x11, 0x22, 0x33, 0x44, 0x55, 0x66, 0x77, 0x88, 0x99, 0xAA, 0xBB, 0xCC, 0xDD,
            0xEE, 0xFF,
        ];
        let expected = [
            0xF9, 0xD9, 0x6E, 0x1D, 0x61, 0x79, 0xA0, 0x94, 0xD1, 0x46, 0x66, 0x71, 0x09, 0xA6,
            0x20, 0x59,
        ];
        assert_eq!(encrypt_block(&key, &block), expected);
    }

    /// Second known-answer vector, on the shipped header session key/IV pair.
    ///
    /// Produced by compiling the repository's own `aes.c` with its own defines
    /// and calling `AES_ECB_encrypt(iv, key, out, 16)` for the header key set
    /// `(1C70F6AC49EA375526614084C803A95F, 8218B2868AAE6A6CC766F22AF4AA6213)`.
    /// A single vector can agree by accident; two independent ones cannot.
    #[test]
    fn matches_the_shipped_primitive_on_the_session_keys() {
        let key = [
            0x1C, 0x70, 0xF6, 0xAC, 0x49, 0xEA, 0x37, 0x55, 0x26, 0x61, 0x40, 0x84, 0xC8, 0x03,
            0xA9, 0x5F,
        ];
        let iv = [
            0x82, 0x18, 0xB2, 0x86, 0x8A, 0xAE, 0x6A, 0x6C, 0xC7, 0x66, 0xF2, 0x2A, 0xF4, 0xAA,
            0x62, 0x13,
        ];
        let expected = [
            0x4C, 0xDB, 0xED, 0x83, 0x8E, 0xB6, 0xA9, 0x2C, 0xDA, 0xB6, 0x7B, 0x61, 0xD8, 0xDC,
            0x66, 0x32,
        ];
        assert_eq!(encrypt_block(&key, &iv), expected);
    }

    /// The table-driven round must equal the byte-wise round on arbitrary input.
    ///
    /// This is the differential oracle for the rewrite. The reference above is
    /// the port the two shipped known-answer vectors pin, so agreement here
    /// means the fast path still produces the product's ciphertext rather than a
    /// self-consistent different one.
    #[test]
    fn table_driven_round_matches_the_byte_wise_reference() {
        let mut random = Lcg(0x5DEE_CE66_D1CE_1234);
        for case in 0..512 {
            let mut key = [0u8; 16];
            let mut block = [0u8; 16];
            random.fill(&mut key);
            random.fill(&mut block);
            let reference = reference_encrypt_block(&key, &block);
            assert_eq!(
                encrypt_block(&key, &block),
                reference,
                "encrypt_block disagrees with the reference on case {case}"
            );
            assert_eq!(
                RoundCipher::new(&key).encrypt(&block),
                reference,
                "RoundCipher disagrees with the reference on case {case}"
            );
            // Structured blocks as well as random ones: uniform and counter
            // patterns exercise the carry and the table edges.
            for seed in [0x00u8, 0xFF, 0x5A] {
                let mut structured = [seed; 16];
                for (index, byte) in structured.iter_mut().enumerate() {
                    *byte = seed ^ (index as u8);
                }
                assert_eq!(
                    encrypt_block(&key, &structured),
                    reference_encrypt_block(&key, &structured),
                    "structured block disagrees (case {case}, seed {seed})"
                );
            }
        }
    }

    /// One expanded key must behave exactly like a per-block expansion.
    ///
    /// The region codec holds one `RoundCipher` for hundreds of thousands of
    /// blocks; this pins that reuse against the one-shot entry point.
    #[test]
    fn an_expanded_key_matches_per_block_expansion() {
        let mut random = Lcg(0x0BAD_C0DE_0BAD_F00D);
        for case in 0..64 {
            let mut key = [0u8; 16];
            random.fill(&mut key);
            let cipher = RoundCipher::new(&key);
            for block_index in 0..64 {
                let mut block = [0u8; 16];
                random.fill(&mut block);
                assert_eq!(
                    cipher.encrypt(&block),
                    encrypt_block(&key, &block),
                    "key reuse disagrees (case {case}, block {block_index})"
                );
            }
        }
    }

    #[test]
    fn keystream_matches_the_shipped_session_keys() {
        let (key_1, iv_1, key_2, iv_2) = crate::crypto::header_session_keys();
        let stream_1 = encrypt_block(&key_1, &iv_1);
        let stream_2 = encrypt_block(&key_2, &iv_2);
        let mut engine = [0u8; 16];
        for index in 0..16 {
            engine[index] = stream_1[index] ^ stream_2[index];
        }
        assert_eq!(hex(&engine), "31d530acb6d67a46c87cc851f5d4c3ca");
        // Encrypted container from the shipped tool for an all-zero body:
        // 639b7ef9e5847a46...  XOR keystream must be "RNNUSR" + zeros.
        let encrypted = [
            0x63, 0x9b, 0x7e, 0xf9, 0xe5, 0x84, 0x7a, 0x46, 0xc8, 0x7c, 0xc8, 0x51, 0xf5, 0xd4,
            0xc3, 0xca,
        ];
        let plain: Vec<u8> = (0..16)
            .map(|index| encrypted[index] ^ stream_1[index] ^ stream_2[index])
            .collect();
        assert_eq!(&plain[..6], b"RNNUSR");
        assert!(plain[6..].iter().all(|byte| *byte == 0));
    }

    /// Optional diagnostic: compare the ported keystream with the one a real
    /// container used, given both the container and its shipped decrypted bytes.
    #[test]
    fn container_keystream_diagnostic() {
        let (Ok(container_path), Ok(plain_path)) = (
            std::env::var("NIOH3_SAVE_CONTAINER"),
            std::env::var("NIOH3_SAVE_PLAIN"),
        ) else {
            return;
        };
        let container = std::fs::read(&container_path).expect("container");
        let plain = std::fs::read(&plain_path).expect("plain");
        let (key_1, iv_1, key_2, iv_2) = crate::crypto::header_session_keys();
        let stream_1 = encrypt_block(&key_1, &iv_1);
        let stream_2 = encrypt_block(&key_2, &iv_2);
        let mut ported = [0u8; 16];
        let mut expected = [0u8; 16];
        for index in 0..16 {
            ported[index] = stream_1[index] ^ stream_2[index];
            expected[index] = container[index] ^ plain[index];
        }
        eprintln!("ported  ={}", hex(&ported));
        eprintln!("expected={}", hex(&expected));
        assert_eq!(
            ported, expected,
            "the ported keystream must match the container"
        );
        let clear = crate::crypto::decrypt_container(&container).expect("decrypt");
        assert_eq!(&clear[..6], b"RNNUSR");
    }

    fn hex(bytes: &[u8]) -> String {
        bytes.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
