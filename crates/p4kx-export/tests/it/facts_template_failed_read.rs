// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, RecordTree, StructValue, Value};
use p4kx_testkit::{TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::super::{BARE, TEMPLATE, database};

const FAILED_TEMPLATE: u128 = 0x2600_0000_0000_4000_8000_0000_0000_0001;
const FAILED_REWARD: u128 = 0x2600_0000_0000_4000_8000_0000_0000_0002;
const FAILED_CALLER: u128 = 0x2600_0000_0000_4000_8000_0000_0000_0003;
const OTHER_CALLER: u128 = 0x2600_0000_0000_4000_8000_0000_0000_0004;
const GENERATOR: u128 = 0x2600_0000_0000_4000_8000_0000_0000_0005;
const BROKEN: u32 = 4;
const LAST_READABLE: u32 = 5;
const FAILED_DUPLICATE: u32 = 6;
const OTHER: u32 = 7;

#[test]
fn a_failed_template_fallback_is_unreadable_and_a_readable_duplicate_wins() -> TestResult {
    let data = database_with_failed_template();
    for input in [Input::Plain, Input::Encrypted, Input::Standalone] {
        let fixture = Fixture::open(&data, input, Vec::new())?;
        check_original(&fixture.entry.open()?)?;
        let (_, catalog) = fixture.diagnostic()?;
        check_missing_values(&catalog, input)?;
        check_readable_duplicate(&catalog, input)?;
        check_attempts(&catalog)?;
        origin::closure(&catalog)?;
    }
    Ok(())
}

fn database_with_failed_template() -> Vec<u8> {
    database()
        .structure(
            TestStruct::new("UnreadableContractTemplate")
                .parent("ContractTemplate")
                .declared_size(128),
        )
        .structure(
            TestStruct::new("UnreadableReward")
                .field("amount", TestType::Int8)
                .declared_size(3),
        )
        .record(broken_template(FAILED_TEMPLATE, "broken"))
        .record(readable_duplicate())
        .record(broken_template(TEMPLATE, "duplicate_broken"))
        .record(TestRecord::new(
            FAILED_REWARD,
            "UnreadableReward",
            "UnreadableReward.Control",
            "libs/foundry/records/reputation/rewards/missionrewards_reputation/control.xml",
        ))
        .record(control_generator())
        .build()
}

fn broken_template(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "UnreadableContractTemplate",
        format!("UnreadableContractTemplate.{name}"),
        format!("libs/foundry/records/contracts/contracttemplates/{name}.xml"),
    )
}

fn readable_duplicate() -> TestRecord {
    let display = TestInstance::new("ContractDisplayInfo").value("illegal", TestValue::Bool(true));
    let settings =
        TestInstance::new("ActiveContractSettings").value("canBeShared", TestValue::Bool(false));
    let class = TestInstance::new("ContractClass_Contract").value(
        "additionalParams",
        TestValue::Array(vec![TestValue::Struct(settings)]),
    );
    TestRecord::new(
        TEMPLATE,
        "ContractTemplate",
        "ContractTemplate.duplicate",
        "libs/foundry/records/contracts/contracttemplates/duplicate.xml",
    )
    .value("contractDisplayInfo", TestValue::Struct(display))
    .value("contractClass", TestValue::Struct(class))
}

fn control_generator() -> TestRecord {
    let contract = |id, name: &str, target| {
        TestValue::Struct(
            TestInstance::new("CareerContract")
                .value("id", TestValue::Guid(id))
                .value("debugName", TestValue::text(name))
                .value("template", TestValue::Reference(target)),
        )
    };
    let handler = TestInstance::new("ContractHandler_Career")
        .value("debugName", TestValue::text("FailedTemplateControl"))
        .value(
            "contracts",
            TestValue::Array(vec![
                contract(FAILED_CALLER, "BrokenTemplate", FAILED_TEMPLATE),
                contract(OTHER_CALLER, "OtherFailedRecord", FAILED_REWARD),
            ]),
        );
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.FailedTemplateControl",
        "libs/foundry/records/contracts/contractgenerator/fallback_source.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn check_original(forge: &DataForge<'_>) -> TestResult {
    for (index, guid) in [
        (BROKEN, FAILED_TEMPLATE),
        (FAILED_DUPLICATE, TEMPLATE),
        (OTHER, FAILED_REWARD),
    ] {
        let record = forge.record(index).ok_or("original failed Record")?;
        assert_eq!(
            record.guid().to_string(),
            health_origin_fixture::guid(guid),
            "the physical failure belongs to this authored nominal GUID"
        );
        assert!(record.tree().is_err(), "the actual whole Record read fails");
    }
    let first = forge.record(0).ok_or("first readable Template")?;
    let last = forge
        .record(LAST_READABLE)
        .ok_or("last readable Template")?;
    assert_eq!(
        first.guid(),
        last.guid(),
        "the two readable Templates share a GUID"
    );
    assert_eq!(
        forge
            .record_by_guid(first.guid())
            .map(|record| record.index()),
        Some(first.index()),
        "the first raw GUID hit differs from the last successful mission dependency"
    );
    let first_tree = first.tree()?;
    let last_tree = last.tree()?;
    check_template_bits(&first_tree, false, true)?;
    check_template_bits(&last_tree, true, false)?;
    let caller = forge.record(8).ok_or("original control Generator")?;
    let caller_tree = caller.tree()?;
    let handler = array_item(caller_tree.root(), "generators", 0)?;
    for (slot, index) in [(0, BROKEN), (1, OTHER)] {
        let contract = array_item(handler, "contracts", slot)?;
        let Some(Value::Reference(reference)) = contract.field("template") else {
            return Err("an authored caller is a real Reference".into());
        };
        let target = reference.original_target().record()?;
        assert_eq!(
            target.index(),
            index,
            "the decoder chose the exact failed physical Record"
        );
    }
    Ok(())
}

fn check_template_bits(tree: &RecordTree<'_>, illegal: bool, share: bool) -> TestResult {
    let display = structure_field(tree.root(), "contractDisplayInfo")?;
    let class = structure_field(tree.root(), "contractClass")?;
    let active = array_item(class, "additionalParams", 0)?;
    assert_eq!(
        (display.field("illegal"), active.field("canBeShared")),
        (Some(&Value::Bool(illegal)), Some(&Value::Bool(share))),
        "the physical Template copy stores the expected original flag pair"
    );
    Ok(())
}

fn check_missing_values(catalog: &Json, input: Input) -> TestResult {
    for (id, state, complete) in [
        (FAILED_CALLER, "unreadable", false),
        (OTHER_CALLER, "absent", true),
    ] {
        let row = catalog_contracts::entry(catalog, &health_origin_fixture::guid(id), None)?;
        let marker = if complete {
            input.marker()
        } else {
            matches!(input, Input::Encrypted).then_some(true)
        };
        for fact in ["canBeShared", "illegal"] {
            let choice = origin::at(row, &format!("/{fact}"))?;
            for member in ["value", "layer"] {
                let cell = origin::cell(choice, member, state)?;
                assert_eq!(
                    cell.get("sourceSetComplete"),
                    Some(&json!(complete)),
                    "{fact}/{member} distinguishes unreadable fallback from typed absence"
                );
                assert_eq!(
                    cell.get("containsEncryptedSource"),
                    Some(&json!(marker)),
                    "{fact}/{member} marks only actual contributions"
                );
                assert!(
                    origin::uses(catalog, cell)?
                        .iter()
                        .all(|used| !is_body_attempt(used, BROKEN)),
                    "the attempted Template body does not contribute to {fact}/{member}"
                );
            }
        }
    }
    Ok(())
}

fn check_readable_duplicate(catalog: &Json, input: Input) -> TestResult {
    let row = catalog_contracts::entry(catalog, &health_origin_fixture::guid(BARE), None)?;
    for (fact, expected) in [("illegal", true), ("canBeShared", false)] {
        let choice = origin::at(row, &format!("/{fact}"))?;
        let value = origin::present(choice, "value", &json!(expected), input.marker())?;
        origin::present(choice, "layer", &json!("template"), input.marker())?;
        let uses = origin::uses(catalog, value)?;
        let index = LAST_READABLE.to_string();
        assert!(
            uses.iter().any(|used| {
                used.get("role").and_then(Json::as_str) == Some("payload")
                    && used.pointer("/location/namespace").and_then(Json::as_str)
                        == Some("dataforgeValue")
                    && used.pointer("/location/indices/0").and_then(Json::as_str)
                        == Some(index.as_str())
                    && used.pointer("/location/field").and_then(Json::as_str) == Some("$value")
            }),
            "{fact} retains its last successfully read physical Template"
        );
    }
    Ok(())
}

fn check_attempts(catalog: &Json) -> TestResult {
    let attempts = origin::rows(catalog, "/provenance/uses")?;
    assert!(
        attempts.iter().any(|used| is_body_attempt(used, BROKEN)),
        "the original selected failed Template body is reported as attempted"
    );
    assert!(
        attempts
            .iter()
            .all(|used| !is_body_attempt(used, FAILED_DUPLICATE) && !is_body_attempt(used, OTHER)),
        "a successful duplicate and a different dependency kind do not supply Template attempts"
    );
    Ok(())
}

fn is_body_attempt(used: &Json, index: u32) -> bool {
    used.get("role").and_then(Json::as_str) == Some("attempted")
        && used.pointer("/location/namespace").and_then(Json::as_str) == Some("dataforgeRecordBody")
        && used.pointer("/location/field").and_then(Json::as_str) == Some("body")
        && used.pointer("/location/indices") == Some(&json!([index.to_string()]))
}

fn structure_field<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Struct(value)) = owner.field(name) else {
        return Err(format!("missing original Struct field {name}").into());
    };
    Ok(value)
}

fn array_item<'read, 'source>(
    owner: &'read StructValue<'source>,
    name: &str,
    slot: usize,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Array(items)) = owner.field(name) else {
        return Err(format!("missing original array {name}").into());
    };
    let Some(Value::Struct(item)) = items.get(slot) else {
        return Err(format!("missing original Struct slot {name}[{slot}]").into());
    };
    Ok(item)
}
