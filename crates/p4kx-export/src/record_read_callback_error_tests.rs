// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, SourceEvidenceError, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalValueEvent;
use crate::guid_text::CatalogGuid;
use crate::selection::Kind;
use crate::template_issues::TemplateError;
use crate::{CatalogError, ExportAccountingError};

#[test]
fn callback_preserves_cancellation_from_actual_original_work() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        original(forge, input)?;
        let cancel = CancelToken::new();
        let result = super::read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            |record, tree| {
                let scope = input.record_scope(record, tree)?;
                cancel.cancel();
                let _ = scope.walk(Some(&cancel)).next_event()?;
                Ok(CatalogGuid(record.guid()))
            },
            |guid: &CatalogGuid| *guid,
        );
        assert!(
            matches!(result, Err(CatalogError::Cancelled)),
            "actual stopped callback work remains a typed fatal cancellation, not a failed source row"
        );
        Ok(())
    })
}

#[test]
fn callback_preserves_the_actual_foreign_input_source_error() -> TestResult {
    let bytes = database();
    test_support::with_input(&bytes, |forge, input| {
        original(forge, input)?;
        test_support::with_input(&bytes, |foreign_forge, foreign| {
            assert!(
                !std::ptr::eq(forge, foreign_forge),
                "genuine separate opened original Forge"
            );
            let result = super::read_kind(
                forge,
                input,
                Kind::ContractTemplates,
                (None, None),
                |record, tree| {
                    foreign.record_scope(record, tree)?;
                    Ok(CatalogGuid(record.guid()))
                },
                |guid: &CatalogGuid| *guid,
            );
            assert!(
                matches!(
                    result,
                    Err(CatalogError::SourceEvidence(
                        SourceEvidenceError::ForeignEvidence
                    ))
                ),
                "the actual owner guard's original typed error is not downgraded to a projection row"
            );
            Ok(())
        })
    })
}

#[test]
fn callback_preserves_accounting_from_actual_same_site_operation() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        original(forge, input)?;
        let result = super::read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            |record, tree| {
                let scope = input.record_scope(record, tree)?;
                let mut walk = scope.walk(None);
                let Some(OriginalValueEvent::Value(site)) = walk.next_event()? else {
                    return Err("actual first original scalar site".into());
                };
                assert!(
                    matches!(site.value(), Value::Bool(false)),
                    "genuine actual Bool, not a fabricated Ref descriptor"
                );
                site.fact().reference_observation()?.capture()?;
                Ok(CatalogGuid(record.guid()))
            },
            |guid: &CatalogGuid| *guid,
        );
        assert!(
            matches!(
                result,
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "real same-site capture rejects the scalar and preserves its typed accounting failure"
        );
        Ok(())
    })
}

#[test]
fn ordinary_projection_error_keeps_the_existing_failed_row() -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        original(forge, input)?;
        let read = super::read_kind(
            forge,
            input,
            Kind::ContractTemplates,
            (None, None),
            |_, tree| {
                let Some(Value::Struct(value)) = tree.field("payload") else {
                    return Err("genuine original domain-error input".into());
                };
                Err(TemplateError::unknown("payload", value).into())
            },
            |guid: &CatalogGuid| *guid,
        )?;
        assert!(
            read.items.is_empty(),
            "domain projection did not invent a row"
        );
        assert_eq!(
            read.failures.len(),
            1,
            "one real source retains ordinary domain failure semantics"
        );
        assert!(
            read.failures
                .first()
                .ok_or("actual domain failure")?
                .reason()
                .contains("payload holds the unknown struct MysteryValue"),
            "original actual source type/cause survives unchanged"
        );
        Ok(())
    })
}

fn original(forge: &DataForge<'_>, input: &OriginalCatalogInput<'_, '_>) -> TestResult {
    assert_eq!(forge.records().len(), 1, "one actual callback source");
    let record = forge.record(0).ok_or("actual original Record")?;
    assert_eq!(
        record.type_name()?,
        "ContractTemplate",
        "real selected kind"
    );
    let tree = record.tree()?;
    assert!(tree.is_from(&record), "actual immutable Record/tree pair");
    assert_eq!(
        tree.error_count(),
        0,
        "all synthetic source facts are readable before callback failure"
    );
    assert_eq!(
        tree.field("flag"),
        Some(&Value::Bool(false)),
        "actual original scalar source"
    );
    input.record_scope(&record, &tree)?;
    Ok(())
}

fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ContractTemplate")
                .field("flag", TestType::Bool)
                .field("payload", TestType::Class("MysteryValue".to_owned())),
        )
        .structure(TestStruct::new("MysteryValue").field("marker", TestType::UInt32))
        .record(
            TestRecord::new(
                0xf190_0000_0000_4000_8000_0000_0000_0001,
                "ContractTemplate",
                "ContractTemplate.CallbackErrors",
                "libs/foundry/records/contracts/contracttemplates/callback_errors.xml",
            )
            .value("flag", TestValue::Bool(false))
            .value(
                "payload",
                TestValue::Struct(
                    TestInstance::new("MysteryValue").value("marker", TestValue::UInt32(7)),
                ),
            ),
        )
        .build()
}
