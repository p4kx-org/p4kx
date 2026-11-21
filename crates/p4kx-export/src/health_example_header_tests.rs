// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataForgeError, SourceLocation, SourceUse, SourceUseRole, SourceValue,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::health_example_retention::RetainedExampleBody;
use crate::catalog_read::original::{ExampleBodyReceipt, RecordSubject};
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

#[derive(Clone, Copy)]
enum Body {
    Readable,
    Malformed,
}

fn bytes(body: Body) -> Vec<u8> {
    let structure = TestStruct::new("HeaderSource").field("value", TestType::UInt32);
    let structure = match body {
        Body::Readable => structure,
        Body::Malformed => structure.declared_size(5),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(structure)
        .record(
            TestRecord::new(41, "HeaderSource", "Header.First", "source/first.xml")
                .value("value", TestValue::UInt32(7)),
        )
        .record(
            TestRecord::new(41, "HeaderSource", "Header.Second", "source/second.xml")
                .value("value", TestValue::UInt32(9)),
        )
        .build()
}

fn subject(
    forge: &DataForge<'_>,
    input: &OriginalCatalogInput<'_, '_>,
    index: u32,
) -> TestResult<RecordSubject> {
    let record = forge
        .record(index)
        .ok_or("the actual original Header Record")?;
    assert_eq!(
        record.index(),
        index,
        "the actual physical Header position is selected independently"
    );
    assert_eq!(
        record.type_name()?,
        "HeaderSource",
        "the actual original source-type getter"
    );
    assert_eq!(
        record.guid().to_string(),
        "00000000-0000-0000-0000-000000000029",
        "the actual stored shared GUID"
    );
    Ok(RecordObservation::capture(input, &record)?
        .subject()
        .clone())
}

#[test]
fn type_only_header_is_genuine_without_requiring_a_successful_body() -> TestResult {
    test_support::with_input(&bytes(Body::Malformed), |forge, input| {
        let record = forge.record(0).ok_or("the actual malformed-body Record")?;
        assert!(
            matches!(
                record.tree(),
                Err(DataForgeError::StructLayout {
                    declared: 5,
                    fields: 4,
                    ..
                })
            ),
            "the independent actual canonical body fails its declared layout"
        );
        let original = subject(forge, input, 0)?;
        let before = input.origins().uses.freeze()?;
        let mut receipt = ExampleBodyReceipt::source_type_header(input, original)?;
        receipt.merge(receipt.clone())?;
        let description = receipt.description()?;
        assert!(
            description.0.is_none() && description.2.is_none(),
            "type-only does not invent field or target"
        );
        assert_eq!(
            description.1.as_deref(),
            Some("HeaderSource"),
            "the original Header supplies the source type"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "capture/clone/description registers no retained Header use"
        );
        let retained = RetainedExampleBody::read(receipt, input)?;
        let texts = retained.texts();
        assert!(
            matches!(
                texts.first().ok_or("required field fact")?.value(),
                SourceValue::Absent
            ),
            "the type-only field remains genuinely absent"
        );
        header(
            texts
                .get(1)
                .ok_or("required source-type fact")?
                .contributions()
                .0,
            0,
            "sourceType",
            SourceUseRole::Selector,
        )?;
        Ok(())
    })
}

#[test]
fn duplicate_header_keeps_its_independent_control_and_own_original_header() -> TestResult {
    test_support::with_input(&bytes(Body::Readable), |forge, input| {
        let original = subject(forge, input, 1)?;
        let before = input.origins().uses.freeze()?;
        let receipt = ExampleBodyReceipt::duplicate_guid_header(input, original)?;
        let description = receipt.description()?;
        assert_eq!(
            description.0.as_deref(),
            Some("$guid"),
            "the field is the fixed duplicate-operation control"
        );
        assert_eq!(
            description.1.as_deref(),
            Some("HeaderSource"),
            "the original Header supplies the source type"
        );
        assert_eq!(
            description.2.map(|guid| guid.to_string()).as_deref(),
            Some("00000000-0000-0000-0000-000000000029"),
            "the actual duplicate Header supplies the target GUID"
        );
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "unretained Header facts add no payload/selector uses"
        );
        let retained = RetainedExampleBody::read(receipt, input)?;
        let texts = retained.texts();
        let (field_uses, complete) = texts
            .first()
            .ok_or("required control field")?
            .contributions();
        assert!(
            complete && field_uses.is_empty(),
            "the fixed control has no fabricated game-source contribution"
        );
        header(
            texts.get(1).ok_or("required type fact")?.contributions().0,
            1,
            "sourceType",
            SourceUseRole::Selector,
        )?;
        let inventory = input.origins().uses.freeze()?;
        let guids: Vec<_> = inventory
            .uses()
            .iter()
            .filter(|used| used.location() == &location(1, "guid"))
            .collect();
        assert_eq!(
            guids.len(),
            1,
            "retention materializes the actual duplicate GUID Header once"
        );
        assert_eq!(
            guids.first().ok_or("original GUID Header use")?.role(),
            SourceUseRole::Payload,
            "the existing original GUID Header contract remains Payload"
        );
        Ok(())
    })
}

#[test]
fn absent_and_accounting_subjects_cannot_authorize_header_recipes() -> TestResult {
    test_support::with_input(&bytes(Body::Readable), |forge, input| {
        let original = subject(forge, input, 0)?;
        let key = original.key().ok_or("the genuine original subject key")?;
        let accounting =
            RecordSubject::accounting(key.index(), key.guid(), key.path_text().cloned());
        let before = input.origins().uses.freeze()?;
        for rejected in [RecordSubject::absent(), accounting] {
            assert!(
                matches!(
                    ExampleBodyReceipt::source_type_header(input, rejected.clone()),
                    Err(CatalogError::Accounting(
                        ExportAccountingError::UnboundHealthSubject
                    ))
                ),
                "only a genuine keyed original Header admits type-only"
            );
            assert!(
                matches!(
                    ExampleBodyReceipt::duplicate_guid_header(input, rejected),
                    Err(CatalogError::Accounting(
                        ExportAccountingError::UnboundHealthSubject
                    ))
                ),
                "only a genuine keyed original Header admits duplicate facts"
            );
        }
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "refused subjects add no uses"
        );
        Ok(())
    })
}

#[test]
fn equal_descriptions_do_not_admit_another_physical_header_or_inventory() -> TestResult {
    test_support::with_input(&bytes(Body::Readable), |forge, input| {
        let original = subject(forge, input, 0)?;
        let mut first = ExampleBodyReceipt::source_type_header(input, original.clone())?;
        let second = ExampleBodyReceipt::source_type_header(input, subject(forge, input, 1)?)?;
        assert_eq!(
            first.description()?,
            second.description()?,
            "visible type-only values genuinely coincide"
        );
        assert!(
            matches!(
                first.merge(second),
                Err(ExportAccountingError::ConflictingSourceDatum)
            ),
            "the same GUID/type cannot authorize another physical Header"
        );
        test_support::with_input(&bytes(Body::Readable), |_, other| {
            assert!(
                !input
                    .origins()
                    .uses
                    .is_same_inventory(&other.origins().uses),
                "the actual preparations have different authorities"
            );
            let before = other.origins().uses.freeze()?;
            assert!(
                matches!(
                    ExampleBodyReceipt::source_type_header(other, original.clone()),
                    Err(CatalogError::Accounting(
                        ExportAccountingError::UnboundHealthSubject
                    ))
                ),
                "a foreign actual Header is refused before capture"
            );
            assert!(
                matches!(
                    first.check_input(other),
                    Err(ExportAccountingError::UnboundHealthSubject)
                ),
                "the retained recipe still requires its exact original input"
            );
            assert_eq!(
                before,
                other.origins().uses.freeze()?,
                "foreign admission contributes no retained facts"
            );
            Ok(())
        })
    })
}

fn location(index: u32, field: &str) -> SourceLocation {
    SourceLocation::Subresource {
        namespace: "dataforgeRecordHeader".to_owned(),
        indices: vec![u64::from(index)],
        field: field.to_owned(),
    }
}

fn header(uses: &[SourceUse], index: u32, field: &str, role: SourceUseRole) -> TestResult {
    assert_eq!(
        uses.len(),
        1,
        "one actual original Header datum contributes"
    );
    let original = uses.first().ok_or("the actual Header use")?;
    assert_eq!(
        original.description().location(),
        &location(index, field),
        "the exact original physical Header coordinate"
    );
    assert_eq!(
        original.role(),
        role,
        "the existing Header use-role contract remains unchanged"
    );
    Ok(())
}
