// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{
    InputScope, SourceInventory, SourceLocation, SourceUseRef, SourceUseRole, ValueError,
};

use crate::build::original_input::test_support::TestResult;
use crate::source_fields::{FieldChecks, FieldError, FieldIssue};

pub(super) fn malformed(checked: &FieldChecks<'_, '_, '_>) -> TestResult {
    let Err(malformed) = checked.text("value") else {
        return Err("genuine Bool must fail String guard".into());
    };
    let malformed = malformed
        .downcast_ref::<FieldError>()
        .ok_or("typed malformed FieldError")?;
    assert!(
        matches!(malformed.description().issue, FieldIssue::Malformed),
        "same exact malformed domain category"
    );
    assert_eq!(
        malformed.description().field,
        "value",
        "same original error position"
    );
    assert!(
        matches!(malformed, FieldError::Captured(captured)
        if matches!(captured.error(), FieldError::Malformed { expected: "a nullable String", .. })),
        "the exact original String requirement remains the direct domain cause"
    );
    assert!(
        malformed.body().is_some(),
        "actual failed guard owns the original receipt"
    );
    assert!(
        matches!(
            malformed
                .source()
                .and_then(|source| source.downcast_ref::<FieldError>()),
            Some(FieldError::Malformed {
                expected: "a nullable String",
                ..
            })
        ),
        "std Error exposes the original malformed field as its immediate cause"
    );
    Ok(())
}

pub(super) fn unreadable(checked: &FieldChecks<'_, '_, '_>) -> TestResult {
    let Err(unreadable) = checked.boolean("bad") else {
        return Err("original bool error cannot become None".into());
    };
    let unreadable = unreadable
        .downcast_ref::<FieldError>()
        .ok_or("typed unreadable FieldError")?;
    assert!(
        matches!(unreadable, FieldError::Captured(captured)
        if matches!(captured.error(), FieldError::Unreadable { source: ValueError::InvalidBool { byte: 2 }, .. })),
        "the exact original bool error remains the cause"
    );
    assert!(
        unreadable.body().is_some(),
        "unreadable original also retains the same-receiver recipe"
    );
    assert!(
        matches!(
            unreadable
                .source()
                .and_then(|source| source.downcast_ref::<FieldError>()),
            Some(FieldError::Unreadable {
                source: ValueError::InvalidBool { byte: 2 },
                ..
            })
        ),
        "std Error exposes the original unreadable field as its immediate cause"
    );
    Ok(())
}

pub(super) fn array_element(checked: &FieldChecks<'_, '_, '_>) -> TestResult {
    let Err(array) = checked.reference_shapes("items") else {
        return Err("actual Bool element must fail Reference guard".into());
    };
    let array = array
        .downcast_ref::<FieldError>()
        .ok_or("typed actual array error")?;
    assert!(
        matches!(array.description().issue, FieldIssue::Malformed),
        "actual element keeps the established malformed category"
    );
    assert_eq!(
        array.description().field,
        "items[0]",
        "actual first array ordinal keeps the established error position"
    );
    assert!(
        array.body().is_some(),
        "actual element binds through its owning declaration and array chain"
    );
    Ok(())
}

pub(super) fn getter_notes(notes: &[FieldError]) -> TestResult {
    assert_eq!(
        notes.len(),
        2,
        "one actual failed guard note per original field"
    );
    for (note, field, issue) in [
        (
            notes.first().ok_or("actual malformed guard")?,
            "value",
            FieldIssue::Malformed,
        ),
        (
            notes.get(1).ok_or("actual unreadable guard")?,
            "bad",
            FieldIssue::Unreadable,
        ),
    ] {
        assert_eq!(
            note.description().field,
            field,
            "same real original error field"
        );
        assert!(
            matches!(
                (note.description().issue, issue),
                (FieldIssue::Malformed, FieldIssue::Malformed)
                    | (FieldIssue::Unreadable, FieldIssue::Unreadable)
            ),
            "same original error category"
        );
        assert!(
            note.body().is_some(),
            "the actual guard already owns its deferred body"
        );
    }
    Ok(())
}

pub(super) fn retained_payload(
    inventory: &SourceInventory,
    id: &SourceUseRef,
    record_index: u32,
) -> TestResult {
    let used = inventory
        .uses()
        .iter()
        .find(|used| used.id() == id)
        .ok_or("actual emitted use")?;
    assert_eq!(
        used.scope(),
        InputScope::Payload,
        "the genuine later retention uses DataForge payload bytes"
    );
    assert_eq!(
        used.role(),
        SourceUseRole::Payload,
        "retention is distinguishable from the baseline metadata"
    );
    assert_eq!(
        used.location(),
        &SourceLocation::Subresource {
            namespace: "dataforgeValue".to_owned(),
            indices: vec![u64::from(record_index), 0],
            field: "$value".to_owned(),
        },
        "the actual retained payload has the exact original position"
    );
    Ok(())
}
