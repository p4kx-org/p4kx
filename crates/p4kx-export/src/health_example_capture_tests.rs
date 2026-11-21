// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::{ExampleBodyCapture, FieldFailureKind};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalValueEvent, OriginalValueSite};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Owner").field("flag", TestType::Bool))
        .record(
            TestRecord::new(0xEC11, "Owner", "Owner.Flag", "original/flag.xml")
                .value("flag", TestValue::Raw(vec![2])),
        )
        .build()
}

#[test]
fn a_second_genuine_read_of_the_same_record_cannot_replace_the_guard_capture_frame() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("original owner")?;
        let tree = record.tree()?;
        let second = record.tree()?;
        assert!(
            tree.is_from(&record) && second.is_from(&record),
            "both real reads have the same physical owner"
        );
        assert_eq!(tree, second, "all visible values coincide");
        let admission = RecordObservation::capture(input, &record)?;
        let scope = input.record_scope(&record, &tree)?;
        let other = input.record_scope(&record, &second)?;
        assert!(
            !std::ptr::eq(&raw const scope, &raw const other),
            "two actual read frames remain distinct"
        );
        let capture = ExampleBodyCapture::new(input, &admission, &scope)?;
        let mut walk = scope.walk(None);
        let mut other_walk = other.walk(None);
        let Some(OriginalValueEvent::Value(site)) = walk.next_event()? else {
            return Err("original value site".into());
        };
        let Some(OriginalValueEvent::Value(other_site)) = other_walk.next_event()? else {
            return Err("second original value site".into());
        };
        assert_original(&site);
        assert_original(&other_site);
        let fact = site.fact();
        let foreign = other_site.fact();
        let receipt =
            capture.field_fact(tree.root(), &fact, &[], "flag", FieldFailureKind::Invalid)?;
        let before = input.origins().uses.freeze()?;
        assert!(
            matches!(
                capture.field_fact(
                    second.root(),
                    &foreign,
                    &[],
                    "flag",
                    FieldFailureKind::Invalid
                ),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "same-Record descriptors cannot substitute the actual captured read"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "rejection materializes no foreign contribution"
        );
        receipt.check_description(Some("flag"), Some("Owner"), None)?;
        let mut clone = receipt.clone();
        clone.merge(receipt)?;
        Ok(())
    })
}

fn assert_original(site: &OriginalValueSite<'_, '_, '_, '_>) {
    assert_eq!(
        site.field().name(),
        "flag",
        "the original selected declaration"
    );
    assert_eq!(
        site.field().storage().data_type(),
        DataType::Bool,
        "actual boolean storage"
    );
    assert!(!site.field().storage().is_array(), "an original scalar");
    assert!(
        matches!(
            site.value(),
            Value::Error(ValueError::InvalidBool { byte: 2 })
        ),
        "the real decoder produces the invalid source fact"
    );
    assert_eq!(site.ordinal(), 0, "the actual original value occurrence");
    assert_eq!(site.parent_ordinal(), None, "the actual root field");
}
