// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use serde_json::{Value, json};

#[path = "model_collision_blob_oracle.rs"]
mod blob;

use blob::{Blob, range_value, word};

use super::super::TestResult;

const TABLE_BEGIN: usize = 80;
const TABLE_COUNT: usize = 40;
const SLOT_LEN: usize = 2 * size_of::<u32>();
const MEASURED_VERSION: u32 = 0x900;

#[derive(Default)]
pub(super) struct Evidence<'a> {
    pub(super) tables: BTreeMap<u64, Table>,
    pub(super) slots: BTreeMap<[u64; 2], Slot<'a>>,
}

pub(super) enum Table {
    Unsupported,
    Malformed,
    Present {
        slot_count: usize,
        table: Range<u64>,
        trailing: Option<Range<u64>>,
    },
}

pub(super) struct Slot<'a> {
    key: u32,
    range: Range<u64>,
    retained: bool,
    pub(super) blob: Blob<'a>,
}

impl<'a> Evidence<'a> {
    pub(super) fn read(bytes: &'a [u8]) -> TestResult<Self> {
        let mut evidence = Self::default();
        let Ok(file) = ChunkFile::parse(bytes) else {
            return Ok(evidence);
        };
        for (ordinal, chunk) in file.chunks().iter().enumerate() {
            if chunk.chunk_type() == ivo::STAT_OBJ_PHYSICS {
                let start = usize::try_from(chunk.offset())?;
                let end = start
                    .checked_add(usize::try_from(chunk.size())?)
                    .ok_or("original extent overflow")?;
                let data = bytes
                    .get(start..end)
                    .ok_or("bounded original collision chunk")?;
                evidence.table(ordinal as u64, chunk.version(), chunk.offset(), data);
            }
        }
        Ok(evidence)
    }

    fn table(&mut self, chunk: u64, version: u32, base: u64, bytes: &'a [u8]) {
        if version != MEASURED_VERSION {
            self.tables.insert(chunk, Table::Unsupported);
            return;
        }
        let Some((end, rows)) = original_table(bytes) else {
            self.tables.insert(chunk, Table::Malformed);
            return;
        };
        let mut cursor = base + end as u64;
        for (ordinal, &[size0, size1, size2, size3, key0, key1, key2, key3]) in
            rows.iter().enumerate()
        {
            let size = u32::from_le_bytes([size0, size1, size2, size3]);
            let key = u32::from_le_bytes([key0, key1, key2, key3]);
            let end = cursor + u64::from(size);
            let data = usize::try_from(cursor - base)
                .ok()
                .zip(usize::try_from(end - base).ok())
                .and_then(|(start, end)| bytes.get(start..end));
            self.slots.insert(
                [chunk, ordinal as u64],
                Slot {
                    key,
                    range: cursor..end,
                    retained: data.is_some(),
                    blob: Blob::read(data, cursor),
                },
            );
            cursor = end;
        }
        let chunk_end = base + bytes.len() as u64;
        self.tables.insert(
            chunk,
            Table::Present {
                slot_count: rows.len(),
                table: base + TABLE_BEGIN as u64..base + end as u64,
                trailing: (cursor <= chunk_end).then_some(cursor..chunk_end),
            },
        );
    }

    pub(super) fn tally(&self) -> Tally {
        let mut tally = Tally {
            tables: self.tables.len(),
            ..Tally::default()
        };
        for slot in self.slots.values() {
            *tally.dispositions.entry(slot.blob.name()).or_default() += 1;
            if let Some(mesh) = slot.blob.mesh() {
                tally.vertices += mesh.vertices();
                tally.triangles += mesh.triangles();
            }
        }
        tally
    }

    pub(super) fn partial(&self) -> bool {
        self.tables.values().any(Table::partial) || self.slots.values().any(Slot::partial)
    }
}

fn original_table(bytes: &[u8]) -> Option<(usize, &[[u8; SLOT_LEN]])> {
    let count = usize::try_from(word(bytes, TABLE_COUNT)?).ok()?;
    let end = TABLE_BEGIN.checked_add(count.checked_mul(SLOT_LEN)?)?;
    let rows = bytes.get(TABLE_BEGIN..end)?.as_chunks::<SLOT_LEN>().0;
    Some((end, rows))
}

impl Table {
    pub(super) fn value(&self) -> Value {
        match self {
            Self::Present {
                slot_count,
                table,
                trailing,
            } => json!({
                "disposition": "read", "slot_count": slot_count, "slot_table": range_value(table.clone()),
                "trailing": trailing.clone().map(range_value),
            }),
            Self::Unsupported => table_state("unsupported-version"),
            Self::Malformed => table_state("malformed"),
        }
    }

    pub(super) const fn partial(&self) -> bool {
        matches!(self, Self::Malformed)
    }
}

fn table_state(disposition: &str) -> Value {
    json!({"disposition": disposition, "slot_count": null, "slot_table": null, "trailing": null})
}

impl Slot<'_> {
    pub(super) const fn partial(&self) -> bool {
        self.blob.is_partial()
    }

    pub(super) fn value(&self) -> Value {
        let [low, high, part_low, part_high] = self.key.to_le_bytes();
        let node = u16::from_le_bytes([low, high]);
        json!({
            "packed_key": self.key,
            "part": u16::from_le_bytes([part_low, part_high]),
            "node": (node != u16::MAX).then_some(node),
            "declared_length": self.range.end - self.range.start,
            "declared_range": range_value(self.range.clone()),
            "retained_range": self.retained.then(|| range_value(self.range.clone())),
            "blob": self.blob.value(),
        })
    }
}

#[derive(Debug, Default)]
pub(super) struct Tally {
    pub(super) tables: usize,
    pub(super) dispositions: BTreeMap<&'static str, usize>,
    pub(super) vertices: usize,
    pub(super) triangles: usize,
}
