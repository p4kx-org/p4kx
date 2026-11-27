// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForge, DataForgeError, Record, RecordTree, SourceEvidenceError, SourceValue, Value,
};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use super::OriginalCatalogInput;
use super::test_support::{self, TestResult};
use crate::CatalogError;
use crate::record_header_field::RecordHeaderField;

#[derive(Clone, Copy)]
enum Body {
    Readable,
    Malformed,
}

fn bytes(body: Body) -> Vec<u8> {
    let owner = TestStruct::new("Owner").field("value", TestType::UInt32);
    let owner = match body {
        Body::Readable => owner,
        Body::Malformed => owner.declared_size(5),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(owner)
        .record(
            TestRecord::new(0x6111, "Owner", "Owner.One", "owner.xml")
                .value("value", TestValue::UInt32(11)),
        )
        .record(
            TestRecord::new(0x6111, "Owner", "Owner.Two", "owner.xml")
                .value("value", TestValue::UInt32(22)),
        )
        .build()
}

#[test]
fn original_catalog_input_rejects_foreign_forge_despite_equal_visible_headers() -> TestResult {
    let bytes = bytes(Body::Readable);
    let foreign = DataForge::open(&bytes)?;
    let other = foreign.record(0).ok_or("actual foreign original Record")?;
    let foreign_tree = other.tree()?;
    test_support::with_input(&bytes, |forge, input| {
        let selected = forge.record(0).ok_or("actual admitted original Record")?;
        equal_headers(&selected, &other)?;
        assert!(
            !other.is_from(forge),
            "the actual foreign Record still has a different Forge"
        );
        own_header(input, &selected)?;
        let before = input.origins().uses.freeze()?;
        foreign_error(&input.record_context(&other));
        foreign_error(&input.record_scope(&other, &foreign_tree));
        foreign_error(&input.record_scope(&selected, &foreign_tree));
        assert_eq!(
            before,
            input.origins().uses.freeze()?,
            "rejected admissions register no fabricated source use"
        );
        Ok(())
    })
}

fn equal_headers(selected: &Record<'_>, other: &Record<'_>) -> TestResult {
    assert_eq!(
        selected.index(),
        other.index(),
        "independent original positions coincide"
    );
    assert_eq!(
        selected.guid(),
        other.guid(),
        "independent original GUIDs coincide"
    );
    assert_eq!(
        selected.record_path()?,
        other.record_path()?,
        "independent normalized paths coincide"
    );
    assert_eq!(
        selected.type_name()?,
        other.type_name()?,
        "independent type labels coincide"
    );
    Ok(())
}

fn own_header(input: &OriginalCatalogInput<'_, '_>, record: &Record<'_>) -> TestResult {
    let context = input.record_context(record)?;
    let own = context.header_selector(
        RecordHeaderField::RecordIndex,
        SourceValue::Present(record.index()),
    )?;
    let own = serde_json::to_value(own)?;
    assert_eq!(
        own.get("containsEncryptedSource"),
        Some(&serde_json::json!(false)),
        "the genuine original plain header is proved"
    );
    Ok(())
}

#[test]
fn admitted_scope_retains_exact_read_frame_and_rejects_same_forge_wrong_record() -> TestResult {
    test_support::with_input(&bytes(Body::Readable), |forge, input| {
        let selected = forge.record(0).ok_or("actual first Record")?;
        let other = forge.record(1).ok_or("actual second Record")?;
        assert_eq!(
            selected.guid(),
            other.guid(),
            "the genuine duplicate GUID cannot authorize another physical Record"
        );
        assert_ne!(
            selected.index(),
            other.index(),
            "the original physical owners remain distinct"
        );
        let tree = selected.tree()?;
        assert_eq!(
            tree.field("value"),
            Some(&Value::UInt32(11)),
            "actual selected original payload"
        );
        let other_tree = other.tree()?;
        assert_eq!(
            other_tree.field("value"),
            Some(&Value::UInt32(22)),
            "actual distinct original payload"
        );
        foreign_error(&input.record_scope(&selected, &other_tree));
        foreign_error(&input.record_scope(&other, &tree));
        retained_frames(input, &selected, &tree)
    })
}

fn retained_frames<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    record: &Record<'source>,
    tree: &RecordTree<'source>,
) -> TestResult {
    let scope = input.record_scope(record, tree)?;
    assert!(
        std::ptr::eq(scope.record(), record),
        "the frame borrows exactly its original Record"
    );
    assert!(
        std::ptr::eq(scope.tree(), tree),
        "the frame borrows exactly its actual canonical read"
    );
    let cloned = (*tree).clone();
    let clone_scope = input.record_scope(record, &cloned)?;
    assert_eq!(
        tree, &cloned,
        "whole genuine clone preserves canonical value equality"
    );
    assert!(
        !std::ptr::eq(scope.tree(), clone_scope.tree()),
        "equal whole trees retain their separate actual frame borrows"
    );
    assert!(
        std::ptr::eq(clone_scope.tree(), &raw const cloned),
        "the genuine cloned frame stays paired with its own whole clone"
    );
    Ok(())
}

#[test]
fn original_header_admission_survives_an_actual_failed_canonical_body_read() -> TestResult {
    test_support::with_input(&bytes(Body::Malformed), |forge, input| {
        let record = forge
            .record(0)
            .ok_or("actual original failed-body Record")?;
        original_failed_body(&record)?;
        let context = input.record_context(&record)?;
        let guid = context.header_value(
            RecordHeaderField::Guid,
            SourceValue::Present(record.guid().to_string()),
        )?;
        let cell = serde_json::to_value(guid)?;
        assert_eq!(
            cell.get("value"),
            Some(&serde_json::json!(record.guid().to_string())),
            "the admitted original GUID retains its exact wire representation"
        );
        assert_eq!(
            cell.get("state"),
            Some(&serde_json::json!("present")),
            "header capture does not invent an unreadable body value"
        );
        assert_eq!(
            cell.get("containsEncryptedSource"),
            Some(&serde_json::json!(false)),
            "only the actual original header contributes"
        );
        assert_eq!(
            cell.get("sourceSetComplete"),
            Some(&serde_json::json!(true)),
            "readable header proof remains complete despite body failure"
        );
        Ok(())
    })
}

fn original_failed_body(record: &Record<'_>) -> TestResult {
    assert!(
        matches!(
            record.tree(),
            Err(DataForgeError::StructLayout {
                declared: 5,
                fields: 4,
                ..
            })
        ),
        "the genuine original body fails its exact declared-versus-field layout contract"
    );
    assert_eq!(
        record.stored_file_name()?,
        "owner.xml",
        "original filename remains readable"
    );
    assert_eq!(
        record.type_name()?,
        "Owner",
        "original type header remains readable"
    );
    Ok(())
}

fn foreign_error<T>(result: &Result<T, CatalogError>) {
    assert!(
        matches!(
            result,
            Err(CatalogError::SourceEvidence(
                SourceEvidenceError::ForeignEvidence
            ))
        ),
        "only the actual original input/Record/tree pair can authorize this read frame"
    );
}
