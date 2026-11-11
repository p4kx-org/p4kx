// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_media::{MediaRecord, Status};
use serde_json::json;

use super::{OWNERS, Origins, TestResult, check_identity};

const CHUNKS: [([u8; 4], u64, u64, u64); 5] = [
    (*b"fmt ", 12, 20, 36),
    (*b"fmt ", 36, 44, 60),
    (*b"data", 60, 68, 76),
    (*b"fmt ", 76, 84, 100),
    (*b"data", 100, 108, 140),
];

pub(super) fn check(records: &[MediaRecord], origins: &Origins) -> TestResult {
    let mut chunks = records
        .iter()
        .enumerate()
        .filter(|(_, row)| row.subresource.namespace == "audio-wem-chunk");
    for owner in OWNERS {
        for (position, (tag, header, body, end)) in CHUNKS.into_iter().enumerate() {
            let (record_index, row) = chunks.next().ok_or("original RIFF chunk")?;
            let ordinal = u64::try_from(position)?;
            check_identity(&row.source, owner)?;
            assert_eq!(row.subresource.indices, [ordinal], "physical chunk index");
            assert_eq!(
                row.status,
                Status::Complete,
                "complete physical chunk geometry"
            );
            assert_eq!(
                row.data,
                json!({
                    "record_type": "wem-chunk",
                    "payload": {
                        "ordinal": ordinal.to_string(),
                        "tag": tag,
                        "header_range": {"basis":{"type":"entry"}, "start":header, "end_exclusive":body},
                        "body_range": {"basis":{"type":"entry"}, "start":body, "end_exclusive":end},
                        "padding_range": null,
                    },
                }),
                "original chunk order and checked source coordinates include post-DATA chunks",
            );
            let receipt = origins.field(
                "records",
                record_index,
                "/data/payload/body_range/end_exclusive",
                owner.payload,
            )?;
            let uses = origins.require_owner(receipt, owner)?;
            assert!(
                uses.iter().any(|used| used.pointer("/location/namespace")
                    == Some(&json!("wemChunk"))
                    && used.pointer("/location/field") == Some(&json!("bodyLength"))
                    && used.pointer("/location/indices") == Some(&json!([body.to_string()]))),
                "body extent comes from the chunk's own length",
            );
        }
    }
    assert!(
        chunks.next().is_none(),
        "only the original selected chunks are emitted"
    );
    Ok(())
}
