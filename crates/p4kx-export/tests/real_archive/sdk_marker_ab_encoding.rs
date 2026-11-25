// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use p4kx_extraction::SourceInventory;
use serde::Serialize;
use serde_json::Value;

use crate::real_archive_catalog_table_parity::TestResult;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SnapshotSizes {
    snapshot_encoding: &'static str,
    encoded_snapshot_bytes: Option<u64>,
    decoded_snapshot_bytes: Option<u64>,
}

impl SnapshotSizes {
    pub(super) const fn none() -> Self {
        Self {
            snapshot_encoding: "none",
            encoded_snapshot_bytes: None,
            decoded_snapshot_bytes: None,
        }
    }
}

#[derive(Serialize)]
struct RowFrame<'row, Row> {
    row: &'row Row,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct InventoryFrame<'inventory> {
    source_inventory: &'inventory SourceInventory,
}

#[derive(Serialize)]
struct TerminalFrame<'terminal> {
    terminal: &'terminal Value,
}

pub(super) struct CountedSnapshot {
    encoder: zstd::stream::write::Encoder<'static, ByteCount>,
    decoded: u64,
}

impl CountedSnapshot {
    pub(super) fn new() -> io::Result<Self> {
        Ok(Self {
            encoder: zstd::stream::write::Encoder::new(ByteCount::default(), 3)?,
            decoded: 0,
        })
    }

    pub(super) fn row(&mut self, row: &impl Serialize) -> TestResult {
        self.line(&RowFrame { row })
    }

    pub(super) fn inventory(&mut self, source_inventory: &SourceInventory) -> TestResult {
        self.line(&InventoryFrame { source_inventory })
    }

    fn line(&mut self, value: &impl Serialize) -> TestResult {
        serde_json::to_writer(&mut *self, value).map_err(|_| "count one SDK snapshot frame")?;
        self.write_all(b"\n")?;
        Ok(())
    }

    pub(super) fn finish(mut self, terminal: &Value) -> TestResult<SnapshotSizes> {
        self.line(&TerminalFrame { terminal })?;
        Ok(SnapshotSizes {
            snapshot_encoding: "ordered-ndjson/zstd-level-3",
            encoded_snapshot_bytes: Some(self.encoder.finish()?.0),
            decoded_snapshot_bytes: Some(self.decoded),
        })
    }
}

impl Write for CountedSnapshot {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        let count = self.encoder.write(bytes)?;
        self.decoded = self
            .decoded
            .checked_add(count as u64)
            .ok_or_else(|| io::Error::other("SDK decoded count overflow"))?;
        Ok(count)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.encoder.flush()
    }
}

#[derive(Default)]
struct ByteCount(u64);

impl Write for ByteCount {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len() as u64)
            .ok_or_else(|| io::Error::other("SDK encoded count overflow"))?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
