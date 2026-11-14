// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::health_origin_fixture::{Input, TestResult};
use crate::health_reference_assertions as references;
use crate::health_reference_fixture as fixture;
use crate::health_reference_source as source;

#[test]
fn health_reference_checks_reject_valid_but_wrong_occurrence_and_text_bindings() -> TestResult {
    let bytes = fixture::database();
    source::original(&bytes)?;
    let catalog = fixture::document(&bytes, Input::Encrypted)?;
    let original = references::by_field(&catalog, "Left[0]")?;
    references::resolved(&catalog, original, "Left[0]", 2, Some(true))?;
    let other = references::by_field(&catalog, "Right[15]")?;
    let mut mutant = original.clone();
    *mutant
        .pointer_mut("/targetRecordPath/sourceUsesRefs")
        .ok_or("actual path refs")? = other
        .pointer("/targetRecordPath/sourceUsesRefs")
        .ok_or("other actual path refs")?
        .clone();
    assert!(
        references::target_binding(&catalog, &mutant, 2).is_err(),
        "an existing other-slot use cannot authorize this datum despite equal text/encryption"
    );
    let mut mutant = original.clone();
    *mutant
        .pointer_mut("/targetRecordPath/value")
        .ok_or("actual path atom")? = original
        .pointer("/recordPath/value")
        .ok_or("actual different owner atom")?
        .clone();
    assert!(
        references::target_binding(&catalog, &mutant, 2).is_err(),
        "a valid but different report-local atom cannot replace the original target spelling"
    );
    Ok(())
}
