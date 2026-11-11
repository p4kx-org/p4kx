// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;

use crate::audio_record_origins as origins;
use crate::origin_contract::OriginProfile;
use crate::{CandidateInventory, DirectoryContext, MediaError, MediaIssue, MediaWriter, Severity};
use crate::{Status, Subresource, origin_lookup};

#[path = "audio_loose_inventory_candidate.rs"]
mod candidate;
#[path = "audio_loose_inventory_payload.rs"]
mod payload;

use self::payload::Inventory;
pub(crate) use self::payload::LooseInventoryPayload;

pub(crate) const CANDIDATE_NAMESPACE: &str = "audio-loose-wem-candidate";
pub(crate) const INVENTORY_NAMESPACE: &str = "audio-loose-wem-inventory";
pub(crate) const CANDIDATE_RECORD: &str = "loose-wem-candidate";
pub(crate) const INVENTORY_RECORD: &str = "loose-wem-inventory";

pub(crate) fn project(
    context: &DirectoryContext<'_, '_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let directory = writer.origins.directory().cloned().ok_or_else(|| {
        MediaError::Contract("loose WEM inventory has no owning Archive origin".into())
    })?;
    let mut observed = 0_u64;
    let inventory = context
        .lookup
        .visit_loose_wems(writer, |candidate, writer| {
            candidate::project(&directory, candidate, writer)?;
            observed += 1;
            Ok(())
        })?;
    let inventory_used = origin_lookup::directory(Some(&directory), writer)?;
    let mut prepared = origins::prepare_for(&directory, OriginProfile::AudioWem)?;
    for field in [
        "/data/payload/observed_candidates",
        "/data/payload/candidates_complete",
    ] {
        origins::field(&mut prepared, field, inventory_used.clone(), writer)?;
    }
    origins::indices(
        &mut prepared,
        crate::origin_contributions::OriginContributions::independent(&directory.owner),
        writer,
    )?;
    let complete = matches!(inventory, CandidateInventory::Complete);
    let (status, issue) = if complete {
        (Status::Complete, None)
    } else {
        origins::subtree(&mut prepared, "/issues", inventory_used, writer)?;
        (
            Status::Partial,
            Some(MediaIssue::new(
                "audio.loose-wem.incomplete-directory",
                "archive-directory",
                Severity::Warning,
                "The owning Archive directory may omit physical loose WEM candidates.",
            )),
        )
    };
    prepared.write(
        writer,
        &Subresource::new(INVENTORY_NAMESPACE, Vec::new()),
        status,
        &LooseInventoryPayload::LooseWemInventory(Inventory {
            observed_candidates: DecimalU64::new(observed),
            candidates_complete: complete,
        }),
        issue.as_slice(),
    )
}
