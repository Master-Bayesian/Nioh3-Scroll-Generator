//! Reuse only the byte-routing plan for an exact ordered address layout.
//! Values are never cached here. Last-write precedence matches the original
//! reverse-writes, stable-sort, first-byte-wins overlay literally.

#[derive(Clone, Copy)]
struct ByteSource {
    address: u64,
    // The low two bits select a little-endian byte; the remaining bits index
    // the word. A valid [(u64, u32)] slice cannot have len * 4 overflow usize.
    index: usize,
}

#[derive(Default)]
pub(crate) struct WriteLayout {
    ready: bool,
    addresses: Vec<u64>,
    sources: Vec<ByteSource>,
}

impl WriteLayout {
    /// Refresh every byte from this candidate. Returns true if the layout had
    /// to be rebuilt. Exact length and ordered address equality are required;
    /// no digest or selected-read projection can license layout reuse.
    pub(crate) fn fill(&mut self, writes: &[(u64, u32)], output: &mut Vec<(u64, u8)>) -> bool {
        let rebuilt = !self.ready
            || self.addresses.len() != writes.len()
            || self
                .addresses
                .iter()
                .zip(writes)
                .any(|(&a, &(b, _))| a != b);
        if rebuilt {
            // If malformed addresses panic under checked-overflow builds, a
            // caller that catches that panic cannot reuse a partial layout.
            self.ready = false;
            self.sources.clear();
            for (word, &(address, _)) in writes.iter().enumerate().rev() {
                for offset in 0..4 {
                    self.sources.push(ByteSource {
                        // Match the baseline's address arithmetic, including
                        // its debug overflow check / release wrap behavior.
                        address: address + offset as u64,
                        index: word * 4 + offset,
                    });
                }
            }
            self.sources.sort_by_key(|source| source.address);
            self.sources.dedup_by_key(|source| source.address);
            self.addresses.clear();
            self.addresses
                .extend(writes.iter().map(|&(address, _)| address));
            self.ready = true;
        }
        output.clear();
        output.extend(self.sources.iter().map(|source| {
            (
                source.address,
                (writes[source.index / 4].1 >> (8 * (source.index % 4))) as u8,
            )
        }));
        rebuilt
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // Frozen pre-patch ReadBounds::prepare_overlay algorithm. Keep its
    // little-endian expansion, reverse order, stable sort and dedup intact.
    fn baseline(writes: &[(u64, u32)]) -> Vec<(u64, u8)> {
        let mut overlay = Vec::new();
        for &(address, word) in writes.iter().rev() {
            for (offset, byte) in word.to_le_bytes().into_iter().enumerate() {
                overlay.push((address + offset as u64, byte));
            }
        }
        overlay.sort_by_key(|&(address, _)| address);
        overlay.dedup_by_key(|(address, _)| *address);
        overlay
    }

    fn check(layout: &mut WriteLayout, output: &mut Vec<(u64, u8)>, writes: &[(u64, u32)]) -> bool {
        let rebuilt = layout.fill(writes, output);
        assert_eq!(*output, baseline(writes), "writes = {writes:?}");
        rebuilt
    }

    #[test]
    fn unaligned_overlaps_keep_the_last_written_byte() {
        let mut layout = WriteLayout::default();
        let mut output = Vec::new();
        let writes = [
            (0x10, 0x4433_2211),
            (0x12, 0x8877_6655),
            (0x11, 0xddcc_bbaa),
        ];
        assert!(check(&mut layout, &mut output, &writes));
        assert_eq!(
            output,
            [
                (0x10, 0x11),
                (0x11, 0xaa),
                (0x12, 0xbb),
                (0x13, 0xcc),
                (0x14, 0xdd),
                (0x15, 0x88)
            ]
        );
        // Fault control: taking the first writer rather than the last changes
        // all overlapping lanes, even though the address set is identical.
        let mut wrong = Vec::new();
        for &(address, word) in &writes {
            for (offset, byte) in word.to_le_bytes().into_iter().enumerate() {
                wrong.push((address + offset as u64, byte));
            }
        }
        wrong.sort_by_key(|&(address, _)| address);
        wrong.dedup_by_key(|(address, _)| *address);
        assert_ne!(output, wrong);
    }

    #[test]
    fn same_layout_refreshes_zero_signed_zero_nan_infinity_and_sentinel_bits() {
        let mut layout = WriteLayout::default();
        let mut output = Vec::new();
        for (i, word) in [0, 0x8000_0000, 0x7fc0_1234, 0xff80_0000, u32::MAX]
            .into_iter()
            .enumerate()
        {
            assert_eq!(
                check(&mut layout, &mut output, &[(100, word), (101, word)]),
                i == 0
            );
        }
    }

    #[test]
    fn every_layout_change_rebuilds_and_absent_bytes_stay_absent() {
        let mut layout = WriteLayout::default();
        let mut output = Vec::new();
        for writes in [
            vec![(10, 1), (12, 2)],
            vec![(12, 2), (10, 1)],
            vec![(12, 2), (11, 1)],
            vec![(12, 2)],
            vec![],
            vec![(0, 0)],
        ] {
            assert!(check(&mut layout, &mut output, &writes));
            assert!(!check(&mut layout, &mut output, &writes));
        }
        assert_eq!(
            output.binary_search_by_key(&4, |&(address, _)| address),
            Err(4)
        );
        assert_eq!(
            output[0],
            (0, 0),
            "an explicit zero remains distinguishable from no byte"
        );
    }

    fn random(state: &mut u64) -> u64 {
        *state ^= *state << 13;
        *state ^= *state >> 7;
        *state ^= *state << 17;
        *state
    }

    #[test]
    fn differential_randomized_layouts_and_reuses_match_every_byte() {
        let mut layout = WriteLayout::default();
        let mut output = Vec::new();
        let mut state = 0x62be_aecc_7182_1345;
        let mut writes = Vec::new();
        for round in 0..20_000 {
            if round % 3 == 0 {
                writes.clear();
                let count = (random(&mut state) % 180) as usize;
                for _ in 0..count {
                    let address = (random(&mut state) % 700) + 0x1_0000_0000;
                    writes.push((address, random(&mut state) as u32));
                }
            } else {
                for (_, word) in &mut writes {
                    *word = random(&mut state) as u32;
                }
                if round % 11 == 0 {
                    writes.reverse();
                }
            }
            check(&mut layout, &mut output, &writes);
        }
    }

    #[test]
    fn address_boundary_behavior_matches_baseline_and_recovers_after_panic() {
        let mut layout = WriteLayout::default();
        let mut output = Vec::new();
        check(&mut layout, &mut output, &[(u64::MAX - 3, 0x0403_0201)]);
        assert_eq!(output.last(), Some(&(u64::MAX, 4)));
        let invalid = [(u64::MAX - 1, 0x0807_0605), (0, 0x4433_2211)];
        let old = std::panic::catch_unwind(|| baseline(&invalid));
        let new = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            layout.fill(&invalid, &mut output);
            output.clone()
        }));
        assert_eq!(
            old.is_err(),
            cfg!(debug_assertions),
            "baseline overflow mode"
        );
        match (old, new) {
            (Ok(expected), Ok(actual)) => assert_eq!(actual, expected),
            (Err(_), Err(_)) => {}
            _ => panic!("address-overflow behavior changed"),
        }
        check(&mut layout, &mut output, &[(u64::MAX - 3, 0x1413_1211)]);
    }
}
