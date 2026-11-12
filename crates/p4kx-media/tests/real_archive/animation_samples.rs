// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;
use std::path::Path;

use p4kx_media::{ByteOrder, ChunkReference, DataType, OffsetBasis};
use serde_json::Value;

use super::shader_accounting::require;
use super::{TestResult, manifests};

pub(super) fn compare(
    base: &Path,
    samples: &Value,
    field: &str,
    expected: &[u8],
    count: usize,
    range: Range<u64>,
) -> TestResult {
    let value = samples.get(field).ok_or("decoded chunk reference")?;
    let chunk: ChunkReference = serde_json::from_value(value.clone())?;
    require(
        matches!(chunk.layout.byte_order, ByteOrder::Little)
            && matches!(
                (field, chunk.layout.dtype),
                ("frames", DataType::U16) | ("values", DataType::F32)
            )
            && chunk.layout.count == count as u64
            && chunk.layout.first == 0
            && chunk.layout.end_exclusive == count as u64,
        "complete typed sample layout",
    )?;
    let extent = chunk.layout.source_range.ok_or("observed encoded range")?;
    require(
        matches!(extent.basis, OffsetBasis::Entry)
            && extent.start == range.start
            && extent.end_exclusive == range.end,
        "sample artifact reports its actual encoded interval",
    )?;
    let artifact = chunk.artifact;
    let bytes = manifests::check_payload(
        base,
        &artifact.path,
        artifact.size,
        &artifact.digest,
        expected.len() as u64,
    )?;
    require(
        bytes == expected,
        "every decoded element agrees with the original parser",
    )
}
