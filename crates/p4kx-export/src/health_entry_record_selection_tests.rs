// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{SourceUseRole, SourceValue};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct};

use super::super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::AuthoredEntryPath;
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::EntryCapture;
use crate::catalog_read::original::health_reference_receipt::ReferenceFrame;
use crate::record_header_field::RecordHeaderField;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner"))
        .record(TestRecord::new(1, "Owner", "Owner.Source", "source.xml"))
        .build()
}

#[test]
fn foreign_genuine_record_selection_is_refused_before_capture() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("the actual original Record")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the original name is read from this physical Record"
        );
        assert_eq!(
            tree.name(),
            "Owner.Source",
            "the full original declaration is preserved"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let frame = Arc::new(ReferenceFrame::read(input, &admission, &scope)?);
        frame.check_input(input)?;
        test_support::with_input(&bytes(), |_, other| {
            distinct_reports(&frame, input, other);
            let first_before = input.origins().uses.freeze()?;
            let other_before = other.origins().uses.freeze()?;
            let outcome = AuthoredEntryPath::captured(
                &EntryCapture::for_input(other),
                tree.name(),
                vec![EntrySelectionUse {
                    source: SelectionSource::Record(Arc::clone(&frame)),
                    location: RecordHeaderField::RecordName.location(record.index()),
                    role: SourceUseRole::Payload,
                }],
            );
            assert!(
                matches!(
                    outcome,
                    Err(CatalogError::Accounting(
                        ExportAccountingError::UnboundHealthSubject
                    ))
                ),
                "the common receiver must refuse a foreign actual frame at admission"
            );
            assert_eq!(
                input.origins().uses.freeze()?,
                first_before,
                "refusal adds no uses to the foreign inventory"
            );
            assert_eq!(
                other.origins().uses.freeze()?,
                other_before,
                "refusal adds no uses to the receiving inventory"
            );
            Ok(())
        })
    })
}

fn distinct_reports(
    frame: &ReferenceFrame,
    input: &OriginalCatalogInput<'_, '_>,
    other: &OriginalCatalogInput<'_, '_>,
) {
    assert!(
        !input
            .origins()
            .uses
            .is_same_inventory(&other.origins().uses),
        "the actual preparations have distinct original inventories"
    );
    assert!(
        matches!(
            frame.check_input(other),
            Err(ExportAccountingError::UnboundHealthSubject)
        ),
        "the existing genuine frame independently proves the foreign inventory"
    );
}

#[test]
fn genuine_own_record_selection_keeps_its_exact_original_name_use() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("the actual original Record")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "the canonical read has its actual physical owner"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let frame = Arc::new(ReferenceFrame::read(input, &admission, &scope)?);
        frame.check_input(input)?;
        let location = RecordHeaderField::RecordName.location(record.index());
        let path = AuthoredEntryPath::captured(
            &EntryCapture::for_input(input),
            tree.name(),
            vec![EntrySelectionUse {
                source: SelectionSource::Record(frame),
                location: location.clone(),
                role: SourceUseRole::Payload,
            }],
        )?;
        path.check_input(input)?;
        let fact = path.retain()?;
        assert!(
            matches!(fact.value(), SourceValue::Present(text) if text.as_str() == tree.name()),
            "the retained text is the actual original Record name"
        );
        let (uses, complete) = fact.contributions();
        assert!(complete, "a genuine own selector is complete");
        assert_eq!(uses.len(), 1, "no unrelated source use is invented");
        let original = uses.first().ok_or("the actual original name use")?;
        assert_eq!(
            original.description().location(),
            &location,
            "the exact original header coordinate is kept"
        );
        assert_eq!(
            original.role(),
            SourceUseRole::Payload,
            "the original role remains distinct from selection/conversion"
        );
        Ok(())
    })
}
