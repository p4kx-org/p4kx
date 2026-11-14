// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::ReferenceObservation;
use crate::catalog_read::original::health_reference_finish::RetainedReference;
use crate::{CatalogError, ExportAccountingError, ExportReferenceStatus};

#[path = "health_reference_receipt_fixture.rs"]
mod fixture;

#[test]
fn equal_reference_values_do_not_admit_a_foreign_inventory() -> TestResult {
    test_support::with_input(&fixture::bytes(), |forge, input| {
        let original = fixture::observation(forge, input)?;
        test_support::with_input(&fixture::bytes(), |other_forge, other_input| {
            let foreign = fixture::observation(other_forge, other_input)?;
            assert!(
                !input
                    .origins()
                    .uses
                    .is_same_inventory(&other_input.origins().uses),
                "the actual original preparations retain different inventories"
            );
            reject_merge(&original, foreign);
            reject_finish(&original, other_input)
        })?;
        finish_original(original, input)
    })
}

#[test]
fn a_shared_inventory_cannot_substitute_another_original_archive_scope() -> TestResult {
    test_support::with_shared_inventory_inputs(
        &fixture::bytes(),
        |forge, input, other_forge, other_input| {
            let original = fixture::observation(forge, input)?;
            let foreign = fixture::observation(other_forge, other_input)?;
            assert!(
                input
                    .origins()
                    .uses
                    .is_same_inventory(&other_input.origins().uses),
                "the genuine sources share one inventory"
            );
            reject_merge(&original, foreign);
            reject_finish(&original, other_input)
        },
    )
}

fn reject_merge(original: &ReferenceObservation, foreign: ReferenceObservation) {
    assert_eq!(original, &foreign, "visible values alone coincide");
    assert!(
        matches!(
            original.merge(foreign),
            Err(ExportAccountingError::ConflictingSourceDatum)
        ),
        "value equality cannot admit a foreign original frame"
    );
}

fn reject_finish(
    original: &ReferenceObservation,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult {
    let before = input.origins().uses.freeze()?;
    let outcome = RetainedReference::read(
        original
            .clone()
            .resolved(ExportReferenceStatus::TargetNotEmitted),
        input,
    );
    assert!(
        matches!(
            outcome,
            Err(CatalogError::Accounting(
                ExportAccountingError::UnboundHealthSubject
            ))
        ),
        "the final report requires its own original input"
    );
    assert_eq!(
        before,
        input.origins().uses.freeze()?,
        "rejection contributes no foreign payload"
    );
    Ok(())
}

fn finish_original(
    original: ReferenceObservation,
    input: &OriginalCatalogInput<'_, '_>,
) -> TestResult {
    let clone = original.clone();
    clone.merge(original)?;
    let retained = RetainedReference::read(
        clone.resolved(ExportReferenceStatus::TargetNotEmitted),
        input,
    )?;
    let facts = retained.texts();
    let texts = input
        .origins()
        .health_texts()
        .seal(&input.origins().uses, None, &facts)?;
    let row = retained.finish(&texts, &input.origins().uses)?;
    assert_eq!(
        row.target_record_index().value(),
        Some(&1),
        "an unchanged own clone retains the original target"
    );
    assert_eq!(
        row.target_record_path_text().value(),
        Some("manufacturers/target"),
        "the text view stays paired to its own cell"
    );
    Ok(())
}
