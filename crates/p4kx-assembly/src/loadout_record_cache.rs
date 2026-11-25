// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::mem::{size_of, size_of_val};
use std::num::NonZeroUsize;
use std::sync::{Arc, Mutex};

use p4kx_dataforge::{DataForge, DataForgeError, Record, RecordTree, Value};

use crate::bounded_read_cache::BoundedReadCache;

/// Reads a canonical tree when the original physical Record belongs to this reader.
pub trait LoadoutRecordReader: Sync {
    /// Returns the complete canonical result, or `None` when the input is outside this reader.
    ///
    /// # Errors
    /// Returns the original canonical construction error for an admitted Record.
    fn read<'read>(
        &'read self,
        record: &Record<'read>,
    ) -> Result<Option<Arc<RecordTree<'read>>>, DataForgeError>;
}

/// A query-owned cache of canonical Record trees from a single DataForge.
pub struct LoadoutRecordCache<'source> {
    forge: &'source DataForge<'source>,
    entries: Mutex<BoundedReadCache<u32, Arc<RecordTree<'source>>>>,
}

impl<'source> LoadoutRecordCache<'source> {
    /// Limits retained trees by owner count and estimated owned storage.
    #[must_use]
    pub fn new(
        forge: &'source DataForge<'source>,
        max_entries: NonZeroUsize,
        max_bytes: usize,
    ) -> Self {
        Self {
            forge,
            entries: Mutex::new(BoundedReadCache::new(max_entries, max_bytes)),
        }
    }
}

impl LoadoutRecordReader for LoadoutRecordCache<'_> {
    fn read<'read>(
        &'read self,
        record: &Record<'read>,
    ) -> Result<Option<Arc<RecordTree<'read>>>, DataForgeError> {
        if !record.is_from(self.forge) {
            return Ok(None);
        }
        let index = record.index();
        if let Ok(entries) = self.entries.lock()
            && let Some(tree) = entries.get(index)
        {
            return Ok(Some(Arc::clone(tree)));
        }
        let Some(original) = self.forge.record(index) else {
            return Ok(None);
        };
        let tree = Arc::new(original.tree()?);
        if let Some(bytes) = retained_bytes(&tree)
            && let Ok(mut entries) = self.entries.lock()
        {
            if let Some(tree) = entries.get(index) {
                return Ok(Some(Arc::clone(tree)));
            }
            entries.insert(index, bytes, Arc::clone(&tree));
        }
        Ok(Some(tree))
    }
}

fn retained_bytes(tree: &RecordTree<'_>) -> Option<usize> {
    let mut bytes = Some(
        size_of_val(tree)
            .saturating_add(size_of_val(tree.fields()))
            .saturating_add(2 * size_of::<usize>()),
    );
    tree.root().visit_owned_values(|site| {
        bytes = bytes.and_then(|count| {
            let owned = match site.value() {
                Value::Struct(value) => size_of_val(value.fields()),
                Value::Array(values) => size_of_val(values.as_ref()),
                Value::Reference(reference) => {
                    let location = reference.location().map_or(0, |location| {
                        size_of_val(location)
                            .saturating_add(size_of_val(location.steps()))
                            .saturating_add(location.as_str().len())
                            .saturating_add(location.steps().len().saturating_mul(size_of::<u64>()))
                            .saturating_add(2 * size_of::<usize>())
                    });
                    size_of_val(reference.as_ref()).saturating_add(location)
                }
                Value::Error(_) => return None,
                _ => 0,
            };
            Some(count.saturating_add(owned))
        });
    });
    bytes
}

#[cfg(test)]
#[path = "loadout_record_cache_tests.rs"]
mod tests;
