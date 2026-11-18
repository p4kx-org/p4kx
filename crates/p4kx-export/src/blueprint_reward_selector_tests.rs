// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Guid, SourceValueState, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{TestResult, fixture, scope_fixture};
use crate::build::original_input::test_support;
use crate::catalog_read::original::{OriginalFlowFields, OriginalValuePart};
use crate::record_observation::RecordObservation;

use crate::catalog_v3::industry_modules::blueprint_source_test_selectors as selectors;

#[test]
fn selected_identity_and_name_fields_retain_their_authored_selectors() -> TestResult {
    for bytes in [fixture::database(3), scope_fixture::database()] {
        test_support::with_same_forge_reports(&bytes, |forge, input, _| {
            let record = forge.record(0).ok_or("original reward owner")?;
            let tree = record.tree()?;
            let admission = RecordObservation::capture(input, &record)?;
            let read = super::super::read(&tree, input, &admission, &fixture::pools(), None)?;
            let scope = input.record_scope(&record, &tree)?;
            let fields = OriginalFlowFields::read(&scope, None)?;
            for (row, expected) in read.rows.iter().zip(&read.interpreted) {
                if let Some(id) = expected.contract_id {
                    identity(row, id, &fields)?;
                } else if let Some(name) = expected.contract_name {
                    let mut found = fields.facts().iter().filter(|fact|
                        matches!(fact.value(), Value::String(value) | Value::Enum(value) if *value == name));
                    let name = found.next().ok_or("unique original fallback name")?;
                    assert!(
                        found.next().is_none(),
                        "fixture names identify one original field"
                    );
                    selectors::contains(row.contract_name(), name, OriginalValuePart::FieldName)?;
                }
            }
            Ok(())
        })?;
    }
    Ok(())
}

fn identity(
    row: &crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource<'_>,
    id: Guid,
    fields: &OriginalFlowFields<'_, '_, '_>,
) -> TestResult {
    let mut found = fields
        .facts()
        .iter()
        .filter(|fact| matches!(fact.value(), Value::Guid(value) if *value == id));
    let id = found.next().ok_or("unique original selected identity")?;
    assert!(
        found.next().is_none(),
        "fixture IDs identify one original field"
    );
    selectors::contains(row.contract_id(), id, OriginalValuePart::FieldName)?;
    selectors::contains(row.contract_name(), id, OriginalValuePart::FieldName)?;
    let owner = match id.parent_ordinal() {
        Some(ordinal) => Some(
            fields
                .facts()
                .get(usize::try_from(ordinal)?)
                .ok_or("original selected identity owner")?,
        ),
        None => None,
    };
    let name = fields
        .field(owner, "debugName")?
        .ok_or("original selected identity name declaration")?
        .1;
    selectors::contains(row.contract_name(), name, OriginalValuePart::FieldName)?;
    Ok(())
}

#[test]
fn structured_identity_rejections_retain_the_actual_concrete_type_selector() -> TestResult {
    test_support::with_same_forge_reports(&structured_identity_database(), |forge, input, _| {
        let record = forge.record(0).ok_or("original owner")?;
        let target = forge.record(1).ok_or("original pool")?;
        let tree = record.tree()?;
        let admission = RecordObservation::capture(input, &record)?;
        let read = super::super::read(
            &tree,
            input,
            &admission,
            &std::collections::HashSet::from([target.guid()]),
            None,
        )?;
        let [row] = read.rows.as_slice() else {
            return Err("one original reward occurrence".into());
        };
        assert_eq!(
            row.contract_id().state(),
            SourceValueState::Absent,
            "a concrete Struct is not a GUID"
        );
        assert_eq!(
            row.contract_name().value(),
            Some(&"fallback"),
            "name-only fallback remains available"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let id = fields
            .field(None, "id")?
            .ok_or("original Struct identity input")?
            .1;
        assert!(
            matches!(id.value(), Value::Struct(_)),
            "the control contains an actual owned Struct"
        );
        selectors::contains(row.contract_id(), id, OriginalValuePart::ConcreteType)?;
        selectors::contains(row.contract_name(), id, OriginalValuePart::ConcreteType)?;
        Ok(())
    })
}

fn structured_identity_database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Unexpected"))
        .structure(TestStruct::new("Pool"))
        .structure(
            TestStruct::new("Owner")
                .field("id", TestType::Class("Unexpected".to_owned()))
                .field("debugName", TestType::String)
                .field("pool", TestType::Reference),
        )
        .record(
            TestRecord::new(1, "Owner", "Owner.Source", "rewards/source.xml")
                .value("id", TestValue::Struct(TestInstance::new("Unexpected")))
                .value("debugName", TestValue::text("fallback"))
                .value("pool", TestValue::Reference(9)),
        )
        .record(TestRecord::new(
            9,
            "Pool",
            "Pool.Target",
            "rewards/pool.xml",
        ))
        .build()
}
