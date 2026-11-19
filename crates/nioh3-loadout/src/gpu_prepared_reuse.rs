//! Exact prepared-contact reuse, scoped to one immutable native MAIN handle.
//!
//! A key is the complete ordered sequence of (address, raw u32 value) writes.
//! Hashes only select buckets: full slice equality decides every match. We do
//! not sort writes, discard duplicate addresses, normalize floats, infer a read
//! set, or use candidate identity as a proxy for the native input. In particular,
//! CPU read certificates that only prove the total cannot certify all channels.

use super::PreparedDamage;
use std::collections::{hash_map::RandomState, HashMap, VecDeque};
use std::hash::BuildHasher;
use std::sync::Arc;
use std::time::Instant;

type Word = (u64, u32);

// At most 16 MiB of word payload on the supported 64-bit host, plus bounded
// table/queue metadata. Large packets may still run; they are simply not kept.
const MAX_ENTRIES: usize = 8192;
const MAX_WORDS: usize = 1024 * 1024;

#[derive(Clone, Copy, Debug, Default, serde::Serialize)]
pub struct PreparedReuseStats {
    pub enabled: bool,
    pub batches: u64,
    pub requested_contacts: u64,
    pub completed_contacts: u64,
    pub submitted_contacts: u64,
    pub submitted_words: u64,
    pub cached_contacts: u64,
    pub coalesced_contacts: u64,
    pub native_calls: u64,
    pub failed_batches: u64,
    pub cache_evictions: u64,
    pub cache_entries: usize,
    pub cache_words: usize,
    pub cache_entry_limit: usize,
    pub cache_word_limit: usize,
    /// Host lookup plus output expansion; excludes the supplied native callback.
    pub reuse_ms: f64,
}

struct ContactCache<S = RandomState> {
    scores: HashMap<Arc<[Word]>, PreparedDamage, S>,
    fifo: VecDeque<Arc<[Word]>>,
    words: usize,
    max_entries: usize,
    max_words: usize,
}

impl<S: BuildHasher> ContactCache<S> {
    fn new(max_entries: usize, max_words: usize, hasher: S) -> Self {
        Self {
            scores: HashMap::with_hasher(hasher),
            fifo: VecDeque::new(),
            words: 0,
            max_entries,
            max_words,
        }
    }

    fn get(&self, words: &[Word]) -> Option<PreparedDamage> {
        self.scores.get(words).copied()
    }

    fn clear(&mut self) {
        self.scores.clear();
        self.fifo.clear();
        self.words = 0;
    }

    fn remember(&mut self, words: &[Word], score: PreparedDamage) -> u64 {
        // Per-contact failures and non-finite component outputs are returned to
        // the caller but never become a cross-batch success certificate.
        if self.max_entries == 0
            || words.len() > self.max_words
            || score.bail != 0
            || score.damage.reserved != 0
            || [
                score.damage.total,
                score.damage.physical,
                score.damage.elemental,
            ]
            .iter()
            .any(|bits| !f32::from_bits(*bits).is_finite())
            || self.scores.contains_key(words)
        {
            return 0;
        }
        let mut evicted = 0;
        while self.scores.len() >= self.max_entries || self.words > self.max_words - words.len() {
            let oldest = self.fifo.pop_front().expect("cache FIFO covers every key");
            self.words -= oldest.len();
            self.scores.remove(oldest.as_ref());
            evicted += 1;
        }
        let key: Arc<[Word]> = Arc::from(words);
        self.words += key.len();
        self.fifo.push_back(Arc::clone(&key));
        self.scores.insert(key, score);
        evicted
    }
}

pub(super) struct PreparedReuse {
    enabled: bool,
    cache: ContactCache,
    stats: PreparedReuseStats,
    failed: Option<String>,
}

impl PreparedReuse {
    pub(super) fn new(enabled: bool) -> Self {
        Self {
            enabled,
            cache: ContactCache::new(MAX_ENTRIES, MAX_WORDS, RandomState::new()),
            stats: PreparedReuseStats::default(),
            failed: None,
        }
    }

    pub(super) fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
        self.clear_cache();
    }

    pub(super) fn clear_cache(&mut self) {
        self.cache.clear();
    }

    pub(super) fn reset_statistics(&mut self) {
        self.stats = PreparedReuseStats::default();
    }

    pub(super) fn stats(&self) -> PreparedReuseStats {
        PreparedReuseStats {
            enabled: self.enabled,
            cache_entries: self.cache.scores.len(),
            cache_words: self.cache.words,
            cache_entry_limit: self.cache.max_entries,
            cache_word_limit: self.cache.max_words,
            ..self.stats
        }
    }

    fn fail(&mut self, message: String) -> String {
        self.stats.failed_batches += 1;
        self.cache.clear();
        self.failed = Some(message.clone());
        message
    }

    /// All-or-nothing expansion: do not publish cached or newly run rows if any
    /// native call or ABI validation fails. A failed handle cannot later answer
    /// a cache-only batch; callers must open a new immutable scene handle.
    pub(super) fn execute<F>(
        &mut self,
        contacts: &[Vec<Word>],
        mut submit: F,
    ) -> Result<Vec<PreparedDamage>, String>
    where
        F: FnMut(&[&[Word]]) -> Result<Vec<PreparedDamage>, String>,
    {
        if let Some(error) = &self.failed {
            return Err(format!(
                "prepared CUDA MAIN handle failed; reopen it: {error}"
            ));
        }
        if contacts.is_empty() {
            return Ok(Vec::new());
        }
        self.stats.batches += 1;
        self.stats.requested_contacts += contacts.len() as u64;
        let planning = Instant::now();
        let mut scores = vec![PreparedDamage::default(); contacts.len()];
        let mut selections = vec![None; contacts.len()];
        let mut unique = Vec::with_capacity(contacts.len());
        let mut pending: HashMap<&[Word], usize> = HashMap::new();
        for (index, words) in contacts.iter().enumerate() {
            if self.enabled {
                if let Some(score) = self.cache.get(words) {
                    scores[index] = score;
                    self.stats.cached_contacts += 1;
                    continue;
                }
                if let Some(&lane) = pending.get(words.as_slice()) {
                    selections[index] = Some(lane);
                    self.stats.coalesced_contacts += 1;
                    continue;
                }
                pending.insert(words.as_slice(), unique.len());
            }
            selections[index] = Some(unique.len());
            unique.push(words.as_slice());
        }
        self.stats.reuse_ms += planning.elapsed().as_secs_f64() * 1000.0;
        if unique.is_empty() {
            self.stats.completed_contacts += contacts.len() as u64;
            return Ok(scores);
        }
        self.stats.native_calls += 1;
        self.stats.submitted_contacts += unique.len() as u64;
        self.stats.submitted_words += unique.iter().map(|words| words.len() as u64).sum::<u64>();
        let results = match submit(&unique) {
            Ok(results) => results,
            Err(error) => return Err(self.fail(error)),
        };
        if results.len() != unique.len() {
            return Err(self.fail("prepared CUDA MAIN returned an invalid result count".into()));
        }
        if results.iter().any(|score| score.damage.reserved != 0) {
            return Err(self.fail("prepared CUDA MAIN returned an invalid component layout".into()));
        }
        let expansion = Instant::now();
        if self.enabled {
            for (words, &score) in unique.iter().zip(&results) {
                self.stats.cache_evictions += self.cache.remember(words, score);
            }
        }
        for (index, selection) in selections.into_iter().enumerate() {
            if let Some(lane) = selection {
                scores[index] = results[lane];
            }
        }
        self.stats.reuse_ms += expansion.elapsed().as_secs_f64() * 1000.0;
        self.stats.completed_contacts += contacts.len() as u64;
        Ok(scores)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::{BuildHasherDefault, Hasher};

    #[derive(Default)]
    struct Collision;
    impl Hasher for Collision {
        fn finish(&self) -> u64 {
            0
        }
        fn write(&mut self, _: &[u8]) {}
    }

    fn answer(total: f32, physical: f32, elemental: f32) -> PreparedDamage {
        PreparedDamage {
            damage: crate::damage::DamageWords {
                total: total.to_bits(),
                physical: physical.to_bits(),
                elemental: elemental.to_bits(),
                reserved: 0,
            },
            bail: 0,
            bail_rva: 0,
            bail_address: 0,
        }
    }

    #[test]
    fn hash_collision_requires_full_ordered_equality() {
        let mut cache = ContactCache::new(8, 80, BuildHasherDefault::<Collision>::default());
        let a = [(8, 1), (12, 2)];
        let b = [(8, 2), (12, 1)];
        let reversed = [(12, 2), (8, 1)];
        cache.remember(&a, answer(1.0, 0.75, 0.25));
        cache.remember(&b, answer(2.0, 1.5, 0.5));
        assert_eq!(cache.get(&a), Some(answer(1.0, 0.75, 0.25)));
        assert_eq!(cache.get(&b), Some(answer(2.0, 1.5, 0.5)));
        assert_eq!(cache.get(&reversed), None);
        assert_eq!(cache.get(&[(8, 1)]), None);
    }

    #[test]
    fn raw_float_bits_and_duplicate_write_order_are_distinct() {
        let mut cache = ContactCache::new(8, 80, RandomState::new());
        let positive_zero = [(8, 0.0_f32.to_bits())];
        let negative_zero = [(8, (-0.0_f32).to_bits())];
        let a = [(8, 1), (8, 2)];
        let b = [(8, 2), (8, 1)];
        for (key, n) in [(&positive_zero[..], 1.0), (&a[..], 2.0)] {
            cache.remember(key, answer(n, n, 0.0));
        }
        assert_eq!(cache.get(&negative_zero), None);
        assert_eq!(cache.get(&b), None);
        assert_eq!(cache.get(&[(8, 0x7fc0_0001)]), None);
    }

    #[test]
    fn batch_coalescing_expands_every_component_in_original_order() {
        let mut reuse = PreparedReuse::new(true);
        let a = vec![(8, 1)];
        let b = vec![(8, 2)];
        let scores = reuse
            .execute(&[a.clone(), b.clone(), a.clone(), b.clone()], |unique| {
                assert_eq!(unique, &[a.as_slice(), b.as_slice()]);
                Ok(vec![answer(3.0, 1.0, 2.0), answer(7.0, 4.0, 3.0)])
            })
            .unwrap();
        assert_eq!(
            scores,
            vec![
                answer(3.0, 1.0, 2.0),
                answer(7.0, 4.0, 3.0),
                answer(3.0, 1.0, 2.0),
                answer(7.0, 4.0, 3.0)
            ]
        );
        let cached = reuse
            .execute(&[b, a], |_| panic!("cache-only batch must not submit"))
            .unwrap();
        assert_eq!(cached, vec![answer(7.0, 4.0, 3.0), answer(3.0, 1.0, 2.0)]);
        assert_eq!(reuse.stats().submitted_contacts, 2);
        assert_eq!(reuse.stats().completed_contacts, 6);
        assert_eq!(reuse.stats().cached_contacts, 2);
        assert_eq!(reuse.stats().coalesced_contacts, 2);
    }

    #[test]
    fn fifo_is_bounded_by_both_entries_and_word_payload() {
        let mut cache = ContactCache::new(2, 3, RandomState::new());
        let score = answer(1.0, 1.0, 0.0);
        assert_eq!(cache.remember(&[(1, 1), (2, 2)], score), 0);
        assert_eq!(cache.remember(&[(3, 3)], score), 0);
        assert_eq!(cache.remember(&[(4, 4)], score), 1);
        assert!(cache.get(&[(1, 1), (2, 2)]).is_none());
        assert_eq!(cache.words, 2);
        assert_eq!(cache.scores.len(), 2);
        assert_eq!(cache.remember(&[(5, 5), (6, 6), (7, 7), (8, 8)], score), 0);
        assert_eq!(
            cache.words, 2,
            "oversized packet must not evict useful keys"
        );
        assert_eq!(cache.remember(&[(5, 5), (6, 6), (7, 7)], score), 2);
        assert_eq!(cache.words, 3);
        assert_eq!(cache.scores.len(), 1);
    }

    #[test]
    fn failure_is_atomic_and_poison_survives_cache_only_retry() {
        let mut reuse = PreparedReuse::new(true);
        let a = vec![(8, 1)];
        let b = vec![(8, 2)];
        reuse
            .execute(std::slice::from_ref(&a), |_| {
                Ok(vec![answer(1.0, 1.0, 0.0)])
            })
            .unwrap();
        assert_eq!(
            reuse.execute(&[a.clone(), b], |_| Err("device failure".into())),
            Err("device failure".into())
        );
        assert_eq!(reuse.stats().cache_entries, 0);
        assert_eq!(reuse.stats().completed_contacts, 1);
        assert!(reuse
            .execute(&[a], |_| panic!("failed handle must not execute"))
            .unwrap_err()
            .contains("reopen"));
        assert!(reuse
            .execute(&[], |_| panic!("failed handle must not execute"))
            .is_err());
    }

    #[test]
    fn invalid_layout_or_short_result_never_populates_cache() {
        for invalid_layout in [false, true] {
            let mut reuse = PreparedReuse::new(true);
            let result = reuse.execute(&[vec![(8, 1)]], |_| {
                if invalid_layout {
                    let mut score = answer(1.0, 1.0, 0.0);
                    score.damage.reserved = 1;
                    Ok(vec![score])
                } else {
                    Ok(Vec::new())
                }
            });
            assert!(result.is_err());
            assert_eq!(reuse.stats().completed_contacts, 0);
            assert_eq!(reuse.stats().cache_entries, 0);
        }
    }

    #[test]
    fn contact_bail_and_nonfinite_components_are_not_retained() {
        let mut reuse = PreparedReuse::new(true);
        let mut bail = answer(0.0, 0.0, 0.0);
        bail.bail = 3;
        bail.bail_rva = 0x1234;
        bail.bail_address = 0x2345;
        let nonfinite = answer(1.0, f32::NAN, 0.0);
        let inputs = vec![vec![(8, 1)], vec![(8, 2)], vec![(8, 1)]];
        let outputs = reuse
            .execute(&inputs, |_| Ok(vec![bail, nonfinite]))
            .unwrap();
        assert_eq!(outputs[0], bail);
        assert_eq!(outputs[2], bail);
        assert_eq!(outputs[1], nonfinite);
        assert_eq!(reuse.stats().cache_entries, 0);
        reuse
            .execute(&inputs, |unique| {
                assert_eq!(unique.len(), 2, "both unsuccessful contacts must rerun");
                Ok(vec![bail, nonfinite])
            })
            .unwrap();
    }

    #[test]
    fn diagnostic_disable_and_cache_reset_do_not_change_bits() {
        let mut reuse = PreparedReuse::new(true);
        reuse.set_enabled(false);
        let inputs = [vec![(8, 1)], vec![(8, 1)]];
        let bits = answer(-0.0, -0.0, 0.0);
        assert_eq!(
            reuse
                .execute(&inputs, |unique| {
                    assert_eq!(unique.len(), 2);
                    Ok(vec![bits, bits])
                })
                .unwrap(),
            vec![bits, bits]
        );
        assert_eq!(reuse.stats().cache_entries, 0);
        reuse.set_enabled(true);
        reuse.execute(&inputs, |_| Ok(vec![bits])).unwrap();
        assert_eq!(reuse.stats().cache_entries, 1);
        reuse.reset_statistics();
        assert_eq!(reuse.stats().submitted_contacts, 0);
        assert_eq!(
            reuse.stats().cache_entries,
            1,
            "statistics reset is not a cold cache reset"
        );
        reuse.clear_cache();
        assert_eq!(reuse.stats().cache_entries, 0);
    }
}
