// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::super::clip_candidates::Fact;
use super::ScopeOutcome;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{DirectoryLookup, MediaError, MediaWriter, origin_error};

pub(super) fn observe(
    lookup: &DirectoryLookup<'_>,
    ordinal: u64,
    outcome: &ScopeOutcome,
    prepared: &PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let ScopeOutcome::Selected { entry_index } = outcome else {
        return Ok(());
    };
    let parameters = usize::try_from(entry_index.get()).map_err(|_| origin_error::counter())?;
    let mut placement = writer.capture_origins();
    for prefix in [
        "/subresource/indices",
        "/data/payload/placement_detail",
        "/data/payload/owner_record_index",
    ] {
        prepared.include_known(prefix, &mut placement)?;
    }
    let ordinal_proof = placement.finish();
    let mut proof = writer.capture_origins();
    proof.include_original(&ordinal_proof)?;
    prepared.include_known("/data/payload/outcome", &mut proof)?;
    lookup.index.animation_clip_candidates.observe(
        Fact::Placement {
            ordinal,
            parameters,
            proof: &proof.finish(),
            ordinal_proof: &ordinal_proof,
        },
        writer,
    )
}
