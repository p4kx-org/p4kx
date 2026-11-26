// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::array;
use std::cell::Cell;
use std::hash::{BuildHasher, RandomState};

use hashbrown::HashTable;
use hashbrown::hash_table::Entry;

use super::UseFacts;

const HASH_SHARDS: usize = 64;
const USES_PER_CHUNK: usize = 1 << 16;

pub(super) struct UseIndex<S = RandomState> {
    rows: UseRows,
    shards: [HashTable<u32>; HASH_SHARDS],
    hash_builder: S,
}

impl<S: BuildHasher + Default> Default for UseIndex<S> {
    fn default() -> Self {
        Self {
            rows: UseRows::default(),
            shards: array::from_fn(|_| HashTable::new()),
            hash_builder: S::default(),
        }
    }
}

impl<S: BuildHasher> UseIndex<S> {
    pub(super) const fn len(&self) -> usize {
        self.rows.len
    }

    pub(super) fn insert(&mut self, id: &str, facts: UseFacts) -> Result<(), &'static str> {
        if id.is_empty() {
            return Err("invalid or duplicate source-use ID");
        }
        let bytes = id.as_bytes();
        let hash = self.hash_builder.hash_one(bytes);
        let shard = self
            .shards
            .get_mut(shard_index(hash))
            .ok_or("invalid source-use hash shard")?;
        let rows = &mut self.rows;
        let hash_builder = &self.hash_builder;
        let missing_key = Cell::new(false);
        let exact_key = |ordinal| {
            let key = rows.id(ordinal);
            if key.is_none() {
                missing_key.set(true);
            }
            key
        };
        let entry = shard.entry(
            hash,
            |&ordinal| exact_key(ordinal) == Some(bytes),
            |&ordinal| exact_key(ordinal).map_or(hash, |key| hash_builder.hash_one(key)),
        );
        if missing_key.get() {
            return Err("an indexed ordinal has no exact source-use key");
        }
        match entry {
            Entry::Occupied(_) => Err("invalid or duplicate source-use ID"),
            Entry::Vacant(entry) => {
                entry.insert(rows.push(bytes, facts)?);
                Ok(())
            }
        }
    }

    pub(super) fn get(&self, id: &str) -> Option<UseFacts> {
        let bytes = id.as_bytes();
        let hash = self.hash_builder.hash_one(bytes);
        let shard = self.shards.get(shard_index(hash))?;
        let ordinal = *shard.find(hash, |&ordinal| self.rows.id(ordinal) == Some(bytes))?;
        self.rows.facts(ordinal)
    }
}

fn shard_index(hash: u64) -> usize {
    let [first, ..] = hash.to_be_bytes();
    usize::from(first) / (256 / HASH_SHARDS)
}

#[derive(Default)]
struct UseRows {
    chunks: Vec<UseChunk>,
    len: usize,
}

#[derive(Default)]
struct UseChunk {
    bytes: Vec<u8>,
    ends: Vec<usize>,
    facts: Vec<UseFacts>,
}

impl UseRows {
    fn push(&mut self, id: &[u8], facts: UseFacts) -> Result<u32, &'static str> {
        let ordinal = u32::try_from(self.len).map_err(|_| "source-use ordinal overflow")?;
        let next_len = self
            .len
            .checked_add(1)
            .ok_or("source-use ordinal overflow")?;
        if self.len.is_multiple_of(USES_PER_CHUNK) {
            self.chunks.push(UseChunk::default());
        }
        let chunk = self.chunks.last_mut().ok_or("missing source-use chunk")?;
        chunk.bytes.extend_from_slice(id);
        chunk.ends.push(chunk.bytes.len());
        chunk.facts.push(facts);
        if chunk.ends.len() == USES_PER_CHUNK {
            chunk.bytes.shrink_to_fit();
        }
        self.len = next_len;
        Ok(ordinal)
    }

    fn at(&self, ordinal: u32) -> Option<(&UseChunk, usize)> {
        let ordinal = usize::try_from(ordinal).ok()?;
        let chunk = self.chunks.get(ordinal / USES_PER_CHUNK)?;
        Some((chunk, ordinal % USES_PER_CHUNK))
    }

    fn id(&self, ordinal: u32) -> Option<&[u8]> {
        let (chunk, row) = self.at(ordinal)?;
        let end = *chunk.ends.get(row)?;
        let start = if row == 0 {
            0
        } else {
            *chunk.ends.get(row - 1)?
        };
        chunk.bytes.get(start..end)
    }

    fn facts(&self, ordinal: u32) -> Option<UseFacts> {
        let (chunk, row) = self.at(ordinal)?;
        chunk.facts.get(row).copied()
    }
}

#[cfg(test)]
#[path = "real_archive_catalog_replay_use_index_tests.rs"]
mod tests;
