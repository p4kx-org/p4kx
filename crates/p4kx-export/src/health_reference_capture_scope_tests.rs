// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Value, ValueError};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::ReferenceCapture;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueEvent, OriginalValueSite};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Clone, Copy)]
enum Method {
    Example,
    Body,
    Enter,
    Reference,
    Error,
}

#[derive(Clone, Copy)]
enum OtherRead {
    SameTree,
    SeparateTree,
}

#[test]
fn example_capture_rejects_a_second_actual_read_of_the_same_record() -> TestResult {
    check(Method::Example, OtherRead::SeparateTree)
}

#[test]
fn example_capture_rejects_a_second_scope_over_the_same_actual_tree() -> TestResult {
    check(Method::Example, OtherRead::SameTree)
}

#[test]
fn body_capture_rejects_a_second_actual_read_of_the_same_record() -> TestResult {
    check(Method::Body, OtherRead::SeparateTree)
}

#[test]
fn container_capture_rejects_a_second_actual_read_of_the_same_record() -> TestResult {
    check(Method::Enter, OtherRead::SeparateTree)
}

#[test]
fn reference_capture_rejects_a_second_actual_read_of_the_same_record() -> TestResult {
    check(Method::Reference, OtherRead::SeparateTree)
}

#[test]
fn error_capture_rejects_a_second_actual_read_of_the_same_record() -> TestResult {
    check(Method::Error, OtherRead::SeparateTree)
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Owner")
                .field("empty", TestType::String)
                .array("values", TestType::UInt8)
                .field("link", TestType::ReferenceTo("Target".to_owned()))
                .field("broken", TestType::ReferenceTo("Target".to_owned())),
        )
        .structure(TestStruct::new("Target"))
        .record(
            TestRecord::new(0xA510, "Owner", "Owner.Source", "original/owner.xml")
                .value("empty", TestValue::text(""))
                .value("values", TestValue::Array(vec![TestValue::UInt8(7)]))
                .value("link", TestValue::Reference(0xA511))
                .value("broken", TestValue::Reference(0xA512)),
        )
        .record(TestRecord::new(
            0xA511,
            "Target",
            "Target.Readable",
            "original/target.xml",
        ))
        .build()
}

fn check(method: Method, other: OtherRead) -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let record = forge.record(0).ok_or("the original owner is absent")?;
        let tree = record.tree()?;
        let second = record.tree()?;
        assert!(
            tree.is_from(&record) && second.is_from(&record),
            "both actual reads have one physical owner"
        );
        assert_eq!(
            tree, second,
            "the second genuine read has equal visible data"
        );
        let scope = input.record_scope(&record, &tree)?;
        let other_tree = match other {
            OtherRead::SameTree => &tree,
            OtherRead::SeparateTree => &second,
        };
        let foreign = input.record_scope(&record, other_tree)?;
        assert!(
            !std::ptr::eq(&raw const scope, &raw const foreign),
            "the exact original scopes are distinct"
        );
        let admission = RecordObservation::capture(input, &record)?;
        let mut capture = ReferenceCapture::new(input, &admission, &scope)?;
        execute(&scope, &mut capture, method)??;
        let before = input.origins().uses.freeze()?;
        let result = execute(&foreign, &mut capture, method)?;
        assert!(
            matches!(
                result,
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "{method:?} must reject a different genuine Scope before retaining a body"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "rejection materializes no foreign contribution"
        );
        execute(&scope, &mut capture, method)??;
        Ok(())
    })
}

fn execute(
    scope: &OriginalRecordScope<'_, '_>,
    capture: &mut ReferenceCapture<'_, '_, '_>,
    method: Method,
) -> TestResult<Result<(), CatalogError>> {
    let wanted = match method {
        Method::Example | Method::Body => "empty",
        Method::Enter => "values",
        Method::Reference => "link",
        Method::Error => "broken",
    };
    let mut walk = scope.walk(None);
    while let Some(event) = walk.next_event()? {
        let OriginalValueEvent::Value(site) = event else {
            continue;
        };
        if site.parent_ordinal().is_some() || site.field().name() != wanted {
            continue;
        }
        original(&site, method);
        return Ok(invoke(capture, &site, method));
    }
    Err("the genuine selected root field is absent".into())
}

fn original(site: &OriginalValueSite<'_, '_, '_, '_>, method: Method) {
    assert_eq!(
        site.owner_type(),
        "Owner",
        "the actual immediate original owner"
    );
    match method {
        Method::Example | Method::Body => assert!(
            matches!(site.value(), Value::String("")),
            "the real empty source text"
        ),
        Method::Enter => assert!(
            matches!(site.value(), Value::Array(values) if values.as_ref() == [Value::UInt8(7)]),
            "the actual original array"
        ),
        Method::Reference => assert!(
            matches!(site.value(), Value::Reference(_)),
            "the actual resolved Reference"
        ),
        Method::Error => assert!(
            matches!(
                site.value(),
                Value::Error(ValueError::DanglingReference { .. })
            ),
            "the actual missing target error"
        ),
    }
}

fn invoke(
    capture: &mut ReferenceCapture<'_, '_, '_>,
    site: &OriginalValueSite<'_, '_, '_, '_>,
    method: Method,
) -> Result<(), CatalogError> {
    match method {
        Method::Example => capture.example(site).map(|_| ()),
        Method::Body => capture.body(site).map(|_| ()),
        Method::Enter => {
            capture.enter(site)?;
            capture.leave(site.ordinal()).map_err(Into::into)
        }
        Method::Reference => capture.reference(site).map(|_| ()),
        Method::Error => capture.error(site).map(|_| ()),
    }
}
