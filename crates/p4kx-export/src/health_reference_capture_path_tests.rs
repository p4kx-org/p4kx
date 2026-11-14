// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Value;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::ReferenceCapture;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueEvent, OriginalValueSite};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
enum Shape {
    Array,
    Struct,
}

#[test]
fn an_active_array_cannot_supply_another_original_array_element() -> TestResult {
    check_parent(Shape::Array)
}

#[test]
fn an_active_struct_cannot_supply_another_struct_with_the_same_type_name() -> TestResult {
    check_parent(Shape::Struct)
}

#[test]
fn a_wrong_container_leave_preserves_the_actual_active_parent() -> TestResult {
    test_support::with_input(&database(Shape::Array), |forge, input| {
        let record = forge.record(0).ok_or("the actual Owner is absent")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let admission = RecordObservation::capture(input, &record)?;
        let mut capture = ReferenceCapture::new(input, &admission, &scope)?;
        let left = enter(&scope, &mut capture, "left")?;
        let right = at(&scope, "right", |site| Ok(site.ordinal()))?;
        active(&scope, &capture, Shape::Array, "left")?;
        let before = input.origins().uses.freeze()?;
        assert!(
            matches!(
                capture.leave(right),
                Err(ExportAccountingError::ConflictingSourceDatum)
            ),
            "another real container ordinal cannot pop the current frame"
        );
        active(&scope, &capture, Shape::Array, "left")?;
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "a refused leave materializes no contribution"
        );
        capture.leave(left)?;
        assert!(
            matches!(
                capture.leave(left),
                Err(ExportAccountingError::ConflictingSourceDatum)
            ),
            "an empty stack cannot complete the same frame again"
        );
        Ok(())
    })
}

fn database(shape: Shape) -> Vec<u8> {
    let owner = match shape {
        Shape::Array => TestStruct::new("Owner")
            .array("left", TestType::String)
            .array("right", TestType::String),
        Shape::Struct => TestStruct::new("Owner")
            .field("left", TestType::Class("Inner".to_owned()))
            .field("right", TestType::Class("Inner".to_owned())),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(owner)
        .structure(TestStruct::new("Inner").field("empty", TestType::String))
        .record(
            TestRecord::new(0xA610, "Owner", "Owner.Paths", "original/paths.xml")
                .value("left", value(shape))
                .value("right", value(shape)),
        )
        .build()
}

fn value(shape: Shape) -> TestValue {
    match shape {
        Shape::Array => TestValue::Array(vec![TestValue::text("")]),
        Shape::Struct => {
            TestValue::Struct(TestInstance::new("Inner").value("empty", TestValue::text("")))
        }
    }
}

fn check_parent(shape: Shape) -> TestResult {
    test_support::with_input(&database(shape), |forge, input| {
        let record = forge.record(0).ok_or("the actual Owner is absent")?;
        let tree = record.tree()?;
        let scope = input.record_scope(&record, &tree)?;
        let admission = RecordObservation::capture(input, &record)?;
        let mut capture = ReferenceCapture::new(input, &admission, &scope)?;
        let left = enter(&scope, &mut capture, "left")?;
        active(&scope, &capture, shape, "left")?;
        let before = input.origins().uses.freeze()?;
        at(&scope, &child(shape, "right"), |site| {
            assert_ne!(
                site.parent_ordinal(),
                Some(left),
                "the genuine right child has another actual parent"
            );
            rejected(
                &capture.example(site),
                "a different actual parent cannot supply a body",
            );
            Ok(())
        })?;
        active(&scope, &capture, shape, "left")?;
        at(&scope, "right", |site| {
            assert_eq!(
                site.parent_ordinal(),
                None,
                "the actual sibling declaration is a root field"
            );
            rejected(
                &capture.enter(site),
                "a root sibling cannot be entered below the active left container",
            );
            Ok(())
        })?;
        active(&scope, &capture, shape, "left")?;
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "wrong-parent rejection materializes no contribution"
        );
        capture.leave(left)?;
        let right = enter(&scope, &mut capture, "right")?;
        active(&scope, &capture, shape, "right")?;
        capture.leave(right)?;
        Ok(())
    })
}

fn rejected<T>(result: &Result<T, CatalogError>, reason: &str) {
    assert!(
        matches!(
            result,
            Err(CatalogError::Accounting(
                ExportAccountingError::ConflictingSourceDatum
            ))
        ),
        "{reason}"
    );
}

fn enter(
    scope: &OriginalRecordScope<'_, '_>,
    capture: &mut ReferenceCapture<'_, '_, '_>,
    path: &str,
) -> TestResult<u64> {
    at(scope, path, |site| {
        assert!(
            matches!(site.value(), Value::Array(_) | Value::Struct(_)),
            "an actual owned container is selected"
        );
        capture.enter(site)?;
        Ok(site.ordinal())
    })
}

fn active(
    scope: &OriginalRecordScope<'_, '_>,
    capture: &ReferenceCapture<'_, '_, '_>,
    shape: Shape,
    path: &str,
) -> TestResult {
    let name = child(shape, path);
    let source_type = match shape {
        Shape::Array => "Owner",
        Shape::Struct => "Inner",
    };
    at(scope, &name, |site| {
        assert!(
            matches!(site.value(), Value::String("")),
            "the real selected child stores empty text"
        );
        assert_eq!(
            site.owner_type(),
            source_type,
            "the actual nearest Struct type"
        );
        capture
            .example(site)?
            .check_description(Some(&name), Some(source_type), None)?;
        Ok(())
    })
}

fn child(shape: Shape, path: &str) -> String {
    match shape {
        Shape::Array => format!("{path}[0]"),
        Shape::Struct => format!("{path}.empty"),
    }
}

fn at<T>(
    scope: &OriginalRecordScope<'_, '_>,
    path: &str,
    check: impl FnOnce(&OriginalValueSite<'_, '_, '_, '_>) -> TestResult<T>,
) -> TestResult<T> {
    let mut walk = scope.walk(None);
    while let Some(event) = walk.next_event()? {
        let OriginalValueEvent::Value(site) = event else {
            continue;
        };
        if site.logical_path() == path {
            return check(&site);
        }
    }
    Err("the demanded genuine original site is absent".into())
}
