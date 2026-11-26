// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, VecDeque};
use std::hash::Hash;
use std::num::NonZeroUsize;

/// Retains completed immutable inputs within an entry and payload budget.
pub(crate) struct BoundedReadCache<K, V> {
    values: HashMap<K, (usize, V)>,
    insertion_order: VecDeque<K>,
    retained_bytes: usize,
    max_entries: NonZeroUsize,
    max_bytes: usize,
}

impl<K, V> std::fmt::Debug for BoundedReadCache<K, V> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("BoundedReadCache")
            .field("entries", &self.values.len())
            .field("retained_bytes", &self.retained_bytes)
            .field("max_entries", &self.max_entries)
            .field("max_bytes", &self.max_bytes)
            .finish_non_exhaustive()
    }
}

impl<K: Copy + Eq + Hash, V> BoundedReadCache<K, V> {
    /// Starts an empty cache with the given entry and payload limits.
    pub(crate) fn new(max_entries: NonZeroUsize, max_bytes: usize) -> Self {
        Self {
            values: HashMap::new(),
            insertion_order: VecDeque::new(),
            retained_bytes: 0,
            max_entries,
            max_bytes,
        }
    }

    /// Borrows one retained input by its exact source key.
    pub(crate) fn get(&self, key: K) -> Option<&V> {
        self.values.get(&key).map(|(_, value)| value)
    }

    pub(crate) fn spare_payload_bytes(&self) -> Option<usize> {
        if self.values.len() >= self.max_entries.get() {
            return None;
        }
        let spare = self.max_bytes - self.retained_bytes;
        (spare > 0).then_some(spare)
    }

    /// Retains a completed input and evicts the oldest entries as needed.
    pub(crate) fn insert(&mut self, key: K, bytes: usize, value: V) {
        if bytes > self.max_bytes || self.values.contains_key(&key) {
            return;
        }
        while self.values.len() >= self.max_entries.get()
            || self.retained_bytes > self.max_bytes - bytes
        {
            let Some(oldest) = self.insertion_order.pop_front() else {
                return;
            };
            if let Some((old_bytes, _)) = self.values.remove(&oldest) {
                self.retained_bytes -= old_bytes;
            }
        }
        self.values.insert(key, (bytes, value));
        self.insertion_order.push_back(key);
        self.retained_bytes += bytes;
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroUsize;

    use super::BoundedReadCache;

    #[test]
    fn retains_completed_inputs_within_both_limits() {
        let mut cache = BoundedReadCache::new(NonZeroUsize::MIN.saturating_add(1), 8);
        cache.insert(1, 2, "one");
        cache.insert(2, 2, "two");
        cache.insert(1, 1, "duplicate");
        assert_eq!(cache.get(1), Some(&"one"));
        cache.insert(3, 2, "three");
        assert_eq!(cache.get(1), None, "entry limit evicts the oldest key");
        assert_eq!(cache.get(2), Some(&"two"));
        cache.insert(4, 9, "over budget");
        assert_eq!(cache.get(4), None, "oversized input is outside the cache");
        assert_eq!(cache.get(3), Some(&"three"));
        cache.insert(5, 8, "five");
        assert_eq!(cache.get(2), None, "byte limit evicts multiple entries");
        assert_eq!(cache.get(3), None);
        assert_eq!(cache.get(5), Some(&"five"));
    }
}
