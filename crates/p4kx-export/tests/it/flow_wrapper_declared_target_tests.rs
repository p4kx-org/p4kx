// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogError, ExportDiagnosticCode, build_catalog_from};
use p4kx_extraction::{DataForge, DataType, Field, StructValue, Value as Raw};
use serde_json::{Value, json};

use super::{Case, fixture, source, wire};
use crate::broker_flow_graph::Broker;
use crate::health_origin_assertions as proof;
use crate::health_origin_fixture::{Input, TestResult};
use crate::loot_v3_graph as raw;
use crate::template_canonical_graph::Template;

#[test]
fn flow_wrappers_reject_wrong_nominal_target_on_populated_multiple_actions() -> TestResult {
    check(
        Case::WrongMultipleTarget,
        "multipleActions",
        "actions",
        DataType::StrongPointer,
    )
}

#[test]
fn flow_wrappers_reject_wrong_nominal_target_on_empty_random_actions() -> TestResult {
    check(
        Case::WrongEmptyPickTarget,
        "pickRandomAction",
        "actions",
        DataType::Class,
    )
}

#[test]
fn flow_wrappers_reject_wrong_nominal_target_on_populated_entry_action() -> TestResult {
    check(
        Case::WrongEntryActionTarget,
        "pickRandomAction",
        "action",
        DataType::StrongPointer,
    )
}

fn check(case: Case, wrapper: &str, field: &str, storage: DataType) -> TestResult {
    let fixture = fixture::open(case, Input::Encrypted, true)?;
    let forge = fixture.entry.open()?;
    let declared = check_original(&forge, case, field, storage)?;
    let ordinary = build_catalog_from(&fixture.archive, &fixture.entry, &source::options());
    assert!(
        matches!(ordinary, Err(CatalogError::IncompleteExport(_))),
        "a correctly shaped but wrongly declared target cannot publish as complete: {wrapper}.{field}"
    );
    let (built, catalog) = fixture.diagnostic()?;
    assert!(
        !built.summary().export_health().is_complete(),
        "the actual authored target error stays visible"
    );
    assert!(
        built
            .summary()
            .export_health()
            .diagnostics()
            .iter()
            .any(|diagnostic| diagnostic.code == ExportDiagnosticCode::UnsupportedValue),
        "the incompatible nominal declaration has its own typed diagnosis"
    );
    let template = Template::new(&catalog, &source::guid(fixture::TEMPLATE))?;
    let broker = Broker::for_guid(&catalog, &source::guid(fixture::BROKER))?;
    for (owner, triggers) in [
        (&template.owner, template.triggers()?),
        (&broker.owner, raw::array(broker.configuration, "triggers")?),
    ] {
        let root = wire::root(owner, triggers)?;
        let node = if field == "action" {
            let entry = proof::at(wire::meaning(root, wrapper)?, "/entries/0/value")?;
            owner.follow(entry)?
        } else {
            root
        };
        let retained = proof::rows(node, "/value/value/fields")?
            .iter()
            .find(|candidate| {
                candidate.pointer("/name/value").and_then(Value::as_str) == Some(field)
            })
            .ok_or("original typed field remains in D")?;
        proof::present(
            proof::at(retained, "/declaration")?,
            "target",
            &json!(declared),
            Some(true),
        )?;
    }
    proof::closure(&catalog)
}

fn check_original(
    forge: &DataForge<'_>,
    case: Case,
    field: &str,
    expected: DataType,
) -> TestResult<u16> {
    let mut target = None;
    for index in [0, 1] {
        let record = forge.record(index).ok_or("actual owner Record")?;
        let tree = record.tree()?;
        assert!(
            tree.is_from(&record),
            "actual physical owner, separately for Template and Broker"
        );
        assert_eq!(
            tree.error_count(),
            0,
            "no generic Record error can explain the required rejection"
        );
        let flow = structure(tree.field("missionFlow").ok_or("original missionFlow")?)?;
        let trigger = structure(only_element(
            flow.field("triggers").ok_or("original triggers")?,
        )?)?;
        let wrapper = structure(trigger.field("action").ok_or("actual wrapper")?)?;
        let owner = if field == "action" {
            structure(only_element(
                wrapper.field("actions").ok_or("actual entry array")?,
            )?)?
        } else {
            wrapper
        };
        let declaration = owner
            .fields()
            .iter()
            .find(|slot| slot.name() == field)
            .ok_or("original declaration")?;
        let storage = declaration.storage();
        assert_eq!(
            storage.data_type(),
            expected,
            "the on-disk storage kind matches the accepted shape"
        );
        assert_eq!(
            storage.is_array(),
            field == "actions",
            "the declaration has the required array shape"
        );
        let actual_target = forge
            .structs()
            .nth(usize::from(storage.target()))
            .ok_or("actual nominal target declaration")?
            .name()?;
        assert_eq!(
            actual_target,
            fixture::OTHER_ENTRY,
            "the actual storage target is a readable but unrelated Struct"
        );
        assert_value(declaration, case)?;
        assert_eq!(
            *target.get_or_insert_with(|| storage.target()),
            storage.target(),
            "both owners use the same actual stored declaration"
        );
    }
    target.ok_or_else(|| "actual declared target word".into())
}

fn assert_value(field: &Field<'_>, case: Case) -> TestResult {
    match case {
        Case::WrongEmptyPickTarget => {
            assert!(
                matches!(field.value(), Raw::Array(values) if values.is_empty()),
                "the Class array is genuinely empty"
            );
        }
        Case::WrongMultipleTarget | Case::WrongEntryActionTarget => {
            let value = if field.storage().is_array() {
                only_element(field.value())?
            } else {
                field.value()
            };
            let actual = structure(value)?;
            assert_eq!(
                actual.type_name(),
                "MissionFlowAction_StateChange",
                "the decoded concrete child is still a known action"
            );
        }
        _ => return Err("unexpected nominal target fixture".into()),
    }
    Ok(())
}

fn only_element<'read, 'source>(value: &'read Raw<'source>) -> TestResult<&'read Raw<'source>> {
    let Raw::Array(values) = value else {
        return Err("actual Array".into());
    };
    let [value] = values.as_ref() else {
        return Err("exactly one original element".into());
    };
    Ok(value)
}

fn structure<'read, 'source>(
    value: &'read Raw<'source>,
) -> TestResult<&'read StructValue<'source>> {
    let Raw::Struct(value) = value else {
        return Err("actual owned Struct".into());
    };
    Ok(value)
}
