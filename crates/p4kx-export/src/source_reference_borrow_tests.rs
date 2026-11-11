// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, SourceValueState, SourcedValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::{OriginalRecordScope, OriginalValueEvent};

use super::super::value::SourcePayload;

const REFERENCES: usize = 64;

#[derive(Clone, Copy)]
enum TargetText {
    RecordPath,
    SourceType,
}

#[test]
fn repeated_loot_references_borrow_the_original_long_target_path() -> TestResult {
    check(TargetText::RecordPath)
}

#[test]
fn repeated_loot_references_borrow_the_original_target_source_type() -> TestResult {
    check(TargetText::SourceType)
}

fn check(field: TargetText) -> TestResult {
    test_support::with_input(&database(), |forge, input| {
        let owner = forge.record(0).ok_or("actual original caller Record")?;
        let target = forge.record(1).ok_or("actual original target Record")?;
        let tree = owner.tree()?;
        assert!(
            tree.is_from(&owner),
            "the canonical tree retains its original owner"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "all actual stored references resolve"
        );
        assert_eq!(
            tree.fields().len(),
            REFERENCES,
            "all genuine distinct caller occurrences exist"
        );
        let scope = input.record_scope(&owner, &tree)?;
        let retained = retain(&scope)?;
        assert_eq!(
            retained.len(),
            REFERENCES,
            "all references were actually projected before checking borrow identity"
        );
        let path = target.record_path()?;
        let source_type = target.type_name()?;
        assert!(
            path.len() > (16 << 10),
            "the original long target path actually exists"
        );
        check_retained(retained, path, source_type, field)
    })
}

fn check_retained(
    retained: Vec<SourcePayload<'_>>,
    path: &str,
    source_type: &str,
    field: TargetText,
) -> TestResult {
    for value in retained {
        let SourcePayload::RecordReference { value } = value else {
            return Err("the actual original Reference retains its typed payload".into());
        };
        let actual_path = text(&value.record_path)?;
        let actual_type = text(&value.target_type)?;
        assert_eq!(actual_path, path, "the original target path remains exact");
        assert_eq!(
            actual_type, source_type,
            "the original concrete target type remains exact"
        );
        let (retained, original) = match field {
            TargetText::RecordPath => (actual_path, path),
            TargetText::SourceType => (actual_type, source_type),
        };
        assert!(
            std::ptr::eq::<str>(retained, original),
            "every retained target string borrows the genuine original bytes without an occurrence clone"
        );
    }
    Ok(())
}

fn retain<'source>(
    scope: &OriginalRecordScope<'_, 'source>,
) -> TestResult<Vec<SourcePayload<'source>>> {
    let mut walk = scope.walk(None);
    let mut retained = Vec::new();
    while let Some(event) = walk.next_event()? {
        let OriginalValueEvent::Value(site) = event else {
            return Err("this actual fixture has only scalar Reference fields".into());
        };
        assert_eq!(
            site.field().storage().data_type(),
            DataType::Reference,
            "every projected value is a genuine stored Reference"
        );
        let Value::Reference(reference) = site.value() else {
            return Err("the original decoder returned an actual resolved Reference".into());
        };
        let admitted = reference.original_target();
        assert!(
            admitted.shares_source_with(scope.record()),
            "the original target shares this admitted input"
        );
        assert_eq!(
            admitted.record_index(),
            1,
            "all distinct source callers select the same real target"
        );
        assert!(
            admitted.instance_location().is_none(),
            "this is a real Record target"
        );
        retained.push(super::read(
            site.fact().reference_observation()?,
            scope.sources(),
        )?);
    }
    Ok(retained)
}

fn text<T: AsRef<str>>(value: &SourcedValue<T>) -> TestResult<&str> {
    assert_eq!(
        value.state(),
        SourceValueState::Present,
        "actual read target text remains present"
    );
    assert_eq!(
        value.origin().contains_encrypted_source(),
        Some(false),
        "actual plain source contribution"
    );
    assert!(
        value.origin().source_set_complete(),
        "actual same-source target capture is complete"
    );
    Ok(value
        .value()
        .ok_or("original present target text")?
        .as_ref())
}

fn database() -> Vec<u8> {
    let mut structure = TestStruct::new("PoolFilterRecord");
    let mut record = TestRecord::new(
        0xe960_0000_0000_4000_8000_0000_0000_0001,
        "PoolFilterRecord",
        "PoolFilterRecord.Owner",
        "owner.xml",
    );
    for ordinal in 0..REFERENCES {
        let name = format!("reference{ordinal}");
        structure = structure.field(&name, TestType::Reference);
        record = record.value(
            name,
            TestValue::Reference(0xe960_0000_0000_4000_8000_0000_0000_0002),
        );
    }
    let path = format!(
        "libs/foundry/records/lootgeneration/filters/long-{}/target.xml",
        "x".repeat(16 << 10)
    );
    DataForgeBuilder::new()
        .version(8)
        .structure(structure)
        .record(record)
        .record(TestRecord::new(
            0xe960_0000_0000_4000_8000_0000_0000_0002,
            "PoolFilterRecord",
            "PoolFilterRecord.Target",
            path,
        ))
        .build()
}
