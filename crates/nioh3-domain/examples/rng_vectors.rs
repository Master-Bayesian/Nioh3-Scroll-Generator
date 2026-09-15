//! TSV emitter for the cross-language RNG parity gate.
//!
//! ```text
//! cargo run --release --quiet --locked --example rng_vectors > rng_vectors.tsv
//! ```
//!
//! Every row is `op<TAB>argument...<TAB>result...` with decimal integers.
//! Binary64 and binary32 inputs travel as raw integer bit patterns, so neither
//! side of the gate parses or formats a text float.
//!
//! Groups and their field order:
//!
//! | op | fields |
//! | --- | --- |
//! | `cvtt` | bits, value |
//! | `threshold` | base, multiplier bits, value |
//! | `lottery` | high16, value |
//! | `lcg` | seed, draws, state, high16 |
//! | `jump` | seed, draw, state |
//! | `mt` | seed, draw, value |
//! | `bounded` | seed, upper, step, value, draws, rejections |
//! | `shuffle` | seed, length, values, draws, rejections |

use nioh3_domain::rng::{
    cvtt_i32, lottery_10000, native_shuffle, state_after, threshold_from_config, LcgStream,
    Mt19937, CONFIG_ROW_BYTES,
};

/// Seeds shared by the `lcg`, `jump`, `mt`, `bounded`, and `shuffle` groups.
const SEEDS: [u32; 6] = [0, 1, 5489, 86872488, 156062997, 4294967295];

/// Binary64 inputs for `cvtt` conversion coverage.
const CVTT_BITS: [u64; 12] = [
    0x0000000000000000,
    0x8000000000000000,
    0x3ff8000000000000,
    0xbff8000000000000,
    0x41dfffffffc00000,
    0x41dfffffffffffff,
    0x41e0000000000000,
    0xc1e0000000000000,
    0xc1e0000000200000,
    0x7ff0000000000000,
    0xfff0000000000000,
    0x7ff8000000000000,
];

/// Base values for `threshold` coverage.
const THRESHOLD_BASES: [i32; 7] = [i32::MIN, -16777217, -1, 0, 1, 16777217, i32::MAX];

/// Binary32 multiplier bit patterns for `threshold` coverage.
const THRESHOLD_MULT_BITS: [u32; 12] = [
    0x00000000, 0x80000000, 0x3f800000, 0x3f7fffff, 0x3f800001, 0x3f000000, 0xbf800000, 0x7f800000,
    0xff800000, 0x7fc00000, 0x00000001, 0x7f7fffff,
];

/// Closed-form jump offsets, including both 2**32 and 2**64-1.
const JUMPS: [u64; 11] = [
    0,
    1,
    2,
    35,
    623,
    624,
    625,
    1248,
    1300,
    4294967296,
    18446744073709551615,
];

/// Inclusive upper bounds, including the singleton and full-width ranges.
const BOUNDS: [u32; 7] = [0, 1, 9, 65535, 2147483648, 4294967294, 4294967295];

/// Shuffle lengths, including empty, singleton, and beyond one MT refill.
const LENGTHS: [usize; 5] = [0, 1, 2, 10, 625];

/// Sequential draws emitted per seed by the `lcg` and `mt` groups.
const DRAWS: u32 = 1300;

/// Consecutive inclusive draws emitted per seed and bound by `bounded`.
const STEPS: u32 = 64;

/// Build the 0x20-byte native configuration row for the emitter: base at 0x10
/// as little-endian `i32`, multiplier at 0x18 as little-endian binary32 bits.
///
/// Row manufacture lives with the vectors that need it; the library only reads
/// captured rows, and the `None`/wrong-length cases stay unit tests.
fn config_row(base: i32, mult_bits: u32) -> [u8; CONFIG_ROW_BYTES] {
    let mut row = [0u8; CONFIG_ROW_BYTES];
    row[0x10..0x14].copy_from_slice(&base.to_le_bytes());
    row[0x18..0x1C].copy_from_slice(&mult_bits.to_le_bytes());
    row
}

fn main() {
    let mut out = String::new();

    for bits in CVTT_BITS {
        let value = cvtt_i32(f64::from_bits(bits));
        out.push_str(&format!("cvtt\t{bits}\t{value}\n"));
    }

    for base in THRESHOLD_BASES {
        for mult_bits in THRESHOLD_MULT_BITS {
            let row = config_row(base, mult_bits);
            let value = threshold_from_config(Some(&row)).expect("0x20-byte row");
            out.push_str(&format!("threshold\t{base}\t{mult_bits}\t{value}\n"));
        }
    }

    // Exhaustive lottery domain: every uint16 ticket.
    for high16 in 0..=u16::MAX {
        out.push_str(&format!("lottery\t{high16}\t{}\n", lottery_10000(high16)));
    }

    // One continuous parent stream per seed, emitting state and draw together.
    for seed in SEEDS {
        let mut stream = LcgStream::new(seed);
        for _ in 0..DRAWS {
            let high16 = stream.u16();
            out.push_str(&format!(
                "lcg\t{seed}\t{}\t{}\t{high16}\n",
                stream.draws(),
                stream.state()
            ));
        }
    }

    for seed in SEEDS {
        for draw in JUMPS {
            out.push_str(&format!(
                "jump\t{seed}\t{draw}\t{}\n",
                state_after(seed, draw)
            ));
        }
    }

    // One continuous MT stream per seed, so rows cross the 624-word refill.
    for seed in SEEDS {
        let mut mt = Mt19937::new(seed);
        for draw in 1..=u64::from(DRAWS) {
            let value = mt.u32();
            out.push_str(&format!("mt\t{seed}\t{draw}\t{value}\n"));
        }
    }

    // A fresh MT per seed and bound, reporting cumulative consumption.
    for seed in SEEDS {
        for upper in BOUNDS {
            let mut mt = Mt19937::new(seed);
            for step in 1..=u64::from(STEPS) {
                let value = mt.inclusive(upper);
                out.push_str(&format!(
                    "bounded\t{seed}\t{upper}\t{step}\t{value}\t{}\t{}\n",
                    mt.draws(),
                    mt.rejections()
                ));
            }
        }
    }

    // A fresh MT per seed and length, shuffling 0..length.
    for seed in SEEDS {
        for length in LENGTHS {
            let mut values: Vec<u32> = (0..length as u32).collect();
            let mut mt = Mt19937::new(seed);
            native_shuffle(&mut values, &mut mt);
            let rendered = values
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(",");
            out.push_str(&format!(
                "shuffle\t{seed}\t{length}\t{rendered}\t{}\t{}\n",
                mt.draws(),
                mt.rejections()
            ));
        }
    }

    print!("{out}");
}
