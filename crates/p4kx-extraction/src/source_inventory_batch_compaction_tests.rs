// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ptr;

use super::SourceInventoryFreezeError;

use crate::{
    InputScope, ProvenanceMode, SourceEvidenceError, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses,
};

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn batch_compaction_retries_temporarily_held_rows_at_projection_end() -> TestResult {
    let uses = SourceUses::with_provenance_mode(ProvenanceMode::Full);
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    let temporary = observe(&scope, 10)?;
    let held = observe(&scope, 2)?;
    let cold = observe(&scope, 100)?;
    let cold_origin = uses.origin(std::slice::from_ref(&cold), true);
    let expected = serde_json::to_vec(&uses.freeze()?)?;
    drop(cold);

    uses.compact_record_use_batch(continue_compaction)?;
    assert_eq!(uses.packed_use_count()?, 1, "only the released row packed");
    drop(temporary);
    uses.compact_record_use_batch(continue_compaction)?;
    assert_eq!(
        uses.packed_use_count()?,
        1,
        "the active batch leaves earlier held IDs deferred"
    );
    uses.compact_record_uses(continue_compaction)?;
    assert_eq!(
        uses.packed_use_count()?,
        2,
        "the temporary hold is released at projection end"
    );

    let original = observe(&scope, 2)?;
    assert!(
        ptr::eq(held.description(), original.description()),
        "a retained observation keeps its authoritative allocation"
    );
    let reopened = observe(&scope, 100)?;
    let cold_id = cold_origin
        .source_uses_refs()
        .first()
        .ok_or("packed source ID")?;
    assert!(
        ptr::eq(reopened.reference().as_str(), cold_id.as_str()),
        "reopening a packed use preserves exactly the captured source ID"
    );
    drop(reopened);
    let finished = uses.finish_with(continue_compaction)?;
    assert_eq!(
        serde_json::to_vec(&finished)?,
        expected,
        "wire inventory and order are exact"
    );
    Ok(())
}

#[test]
fn interruption_retains_both_unvisited_and_deferred_uses() -> TestResult {
    let uses = SourceUses::with_provenance_mode(ProvenanceMode::Full);
    let scope = uses.observe(SourceInput::Unknown, InputScope::Payload)?;
    drop(observe(&scope, 1)?);
    let held = observe(&scope, 2)?;
    let expected = serde_json::to_vec(&uses.freeze()?)?;

    let mut checkpoints = 0;
    let interrupted = uses.compact_record_use_batch(|| {
        checkpoints += 1;
        if checkpoints == 2 {
            Err(SourceInventoryFreezeError::Interrupted(()))
        } else {
            Ok(())
        }
    });
    assert!(
        matches!(
            interrupted,
            Err(SourceInventoryFreezeError::Interrupted(()))
        ),
        "the caller's interruption propagates out of the batch"
    );
    assert_eq!(
        uses.registered_use_count()?,
        2,
        "interruption retains both registrations"
    );
    drop(held);
    uses.compact_record_uses(continue_compaction)?;
    assert_eq!(
        uses.packed_use_count()?,
        2,
        "the interrupted batch retained both candidates"
    );
    assert_eq!(
        serde_json::to_vec(&uses.finish_with(continue_compaction)?)?,
        expected,
        "resuming the interrupted scan preserves the wire inventory"
    );
    Ok(())
}

fn observe(scope: &SourceScope, index: u32) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::DataForge {
            record_index: index,
            field: "source".into(),
        },
        SourceUseRole::Payload,
    )
}

const fn continue_compaction() -> Result<(), SourceEvidenceError> {
    Ok(())
}
