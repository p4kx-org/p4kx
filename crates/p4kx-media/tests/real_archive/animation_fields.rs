// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{AnimationMetadata, DbaAnimationName};
use serde_json::{Value, json};

use super::{
    Artifact, ByteOrder, ChunkReference, DataType, MediaRecord, OffsetBasis, TestResult, check,
    range, require,
};

pub(super) fn reference(
    samples: &Value,
    field: &str,
    count: usize,
    range: Range<u64>,
) -> TestResult<Artifact> {
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
    Ok(chunk.artifact)
}

pub(super) fn metadata(
    row: &MediaRecord,
    metadata: AnimationMetadata<'_>,
    name: Option<DbaAnimationName<'_>>,
) -> TestResult {
    let database_name = name.map(|name| {
        json!({
            "utf8": std::str::from_utf8(name.bytes()).ok(),
            "range": range(&name.range())
        })
    });
    let timing = metadata.timing().ok().map(|timing| {
        json!({
            "frames_per_second": timing.frames_per_second,
            "end_frame": timing.end_frame,
            "additive": timing.additive
        })
    });
    for (field, expected) in [
        (
            "/payload/metadata_bytes",
            json!(metadata.bytes().as_slice()),
        ),
        ("/payload/flags", json!(metadata.flags())),
        ("/payload/metadata_chunk_version", json!(metadata.version())),
        ("/payload/metadata_range", range(&metadata.range())),
        ("/payload/database_name", json!(database_name)),
        ("/payload/timing", json!(timing)),
    ] {
        check(&row.data, field, &expected)?;
    }
    Ok(())
}
