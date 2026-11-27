// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Wem;
use p4kx_media::MediaRecord;
use serde_json::json;

use super::{TestResult, at, range, require};

pub(super) fn verify(original: &[u8], rows: &[MediaRecord]) -> TestResult {
    let wem = Wem::parse(original)?;
    let mut summaries = at(rows, "audio-wem");
    let summary = summaries.next().ok_or("standalone WEM summary")?;
    require(summaries.next().is_none(), "one standalone WEM summary")?;
    require(
        summary.data.pointer("/payload/format_tag") == Some(&json!(wem.format_tag())),
        "original WEM format tag",
    )?;
    let actual = at(rows, "audio-wem-chunk").collect::<Vec<_>>();
    let expected = wem.chunks().collect::<Vec<_>>();
    require(
        actual.len() == expected.len(),
        "complete RIFF/RIFX directory",
    )?;
    for (ordinal, (row, chunk)) in actual.into_iter().zip(expected).enumerate() {
        require(
            row.subresource.indices == [u64::try_from(ordinal)?],
            "original WEM chunk position",
        )?;
        require(
            row.data.pointer("/payload/tag") == Some(&json!(chunk.id)),
            "original WEM tag",
        )?;
        require(
            range(row, "/payload/body_range", original)? == chunk.body,
            "original WEM chunk body",
        )?;
        require(
            range(row, "/payload/header_range", original)?.starts_with(&chunk.id),
            "original WEM chunk header",
        )?;
    }
    Ok(())
}
