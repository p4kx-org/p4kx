// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, StructValue, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value as Json, json};

use crate::catalog_contracts;
use crate::health_origin_assertions as origin;
use crate::health_origin_fixture::{self, Fixture, Input, TestResult};

use super::super::{GATED, GENERATOR, INTRO, TAG};

const REQUIREMENT: &str = "ContractPrerequisite_CompletedContractTags";
const COMPLETION: &str = "ContractResult_CompletionTags";
const FIRST_TAG: &str = "Tag.First";
const SECOND_TAG: &str = "Tag.Second";

struct Targets {
    requirement: u32,
    grant: u32,
}

#[test]
fn weak_intro_selection_keeps_the_exact_target_guid_headers() -> TestResult {
    let bytes = fixture_bytes();
    let fixture = Fixture::open(&bytes, Input::Encrypted, Vec::new())?;
    let targets = original_targets(&fixture)?;
    let (_, catalog) = fixture.diagnostic()?;
    let row = catalog_contracts::entry(&catalog, &health_origin_fixture::guid(GATED), None)?;
    let requirements = origin::cell(row, "requiredIntros", "present")?;
    let [requirement] = origin::rows(requirements, "/value")? else {
        return Err("the actual authored weak requirement survives".into());
    };
    let tag = origin::present(
        requirement,
        "tagGuid",
        &json!(health_origin_fixture::guid(TAG)),
        Some(true),
    )?;
    let grants = origin::cell(requirement, "introContracts", "present")?;
    let [grant] = origin::rows(grants, "/value")? else {
        return Err(
            "the different physical Tag's equal logical GUID grants this requirement".into(),
        );
    };
    assert_eq!(
        grant.get("value"),
        Some(&json!(health_origin_fixture::guid(INTRO))),
        "legacy logical tag matching retains the intro ID"
    );
    assert_header(&catalog, tag, targets.requirement, "payload")?;
    assert_header(&catalog, requirements, targets.requirement, "selector")?;
    assert!(
        origin::uses(&catalog, requirements)?
            .iter()
            .all(|used| !at_header(used, targets.grant)),
        "the grant's same-GUID Tag does not choose the authored requirement"
    );
    for cell in [grants, grant] {
        assert_header(&catalog, cell, targets.grant, "selector")?;
    }
    origin::closure(&catalog)
}

fn original_targets(fixture: &Fixture) -> TestResult<Targets> {
    let forge = fixture.entry.open()?;
    let owner = forge.record(2).ok_or("actual Generator Record")?;
    let tree = owner.tree()?;
    assert!(tree.is_from(&owner), "actual original Generator tree");
    let handler = first(tree.root(), "generators")?;
    let contract = first(handler, "contracts")?;
    let prerequisite = first(contract, "additionalPrerequisites")?;
    let list = structure(prerequisite.field("requiredCompletedContractTags"))?;
    let intro = first(handler, "introContracts")?;
    let results = structure(intro.field("contractResults"))?;
    let outcome = first(results, "contractResults")?;
    let member = first(outcome, "completionTags")?;
    let requirement = weak_record(list, "tags")?;
    let grant = weak_record(member, "tag")?;
    assert_eq!(
        (requirement, grant),
        (0, 1),
        "the two original weak pointers select different physical Tag Records"
    );
    Ok(Targets { requirement, grant })
}

fn weak_record(parent: &StructValue<'_>, name: &str) -> TestResult<u32> {
    let field = parent
        .fields()
        .iter()
        .find(|field| field.name() == name)
        .ok_or("actual weak pointer field")?;
    assert_eq!(
        field.storage().data_type(),
        DataType::WeakPointer,
        "the original declaration stores a pointer, not a GUID"
    );
    let value = match field.value() {
        Value::Array(values) => values.first().ok_or("actual original pointer slot")?,
        scalar => scalar,
    };
    let Value::Reference(reference) = value else {
        return Err("the actual weak pointer resolves to a Record".into());
    };
    assert!(
        reference.location().is_none(),
        "a whole-Record weak target participates in the legacy join"
    );
    let admitted = reference.original_target().record()?;
    assert_eq!(
        admitted.guid().to_string(),
        health_origin_fixture::guid(TAG),
        "the selected physical Record carries the common logical GUID"
    );
    Ok(admitted.index())
}

fn first<'read, 'source>(
    parent: &'read StructValue<'source>,
    name: &str,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Array(values)) = parent.field(name) else {
        return Err("actual original array".into());
    };
    structure(values.first())
}

fn structure<'read, 'source>(
    value: Option<&'read Value<'source>>,
) -> TestResult<&'read StructValue<'source>> {
    let Some(Value::Struct(value)) = value else {
        return Err("actual original Struct occurrence".into());
    };
    Ok(value)
}

fn assert_header(catalog: &Json, cell: &Json, index: u32, role: &str) -> TestResult {
    assert!(
        origin::uses(catalog, cell)?
            .iter()
            .any(|used| at_header(used, index) && used.get("role") == Some(&json!(role))),
        "the original Tag's physical GUID header at Record {index} supplies the {role}"
    );
    Ok(())
}

fn at_header(used: &Json, index: u32) -> bool {
    used.pointer("/location/namespace").and_then(Json::as_str) == Some("dataforgeRecordHeader")
        && used.pointer("/location/indices") == Some(&json!([index.to_string()]))
        && used.pointer("/location/field").and_then(Json::as_str) == Some("guid")
}

fn fixture_bytes() -> Vec<u8> {
    let class = |name: &str| TestType::Class(name.to_owned());
    let weak = || TestType::WeakPointer("Tag".to_owned());
    let structures = [
        TestStruct::new("Tag").field("tagName", TestType::String),
        TestStruct::new("ContractGenerator").array("generators", class("WeakIntroHandler")),
        TestStruct::new("WeakIntroHandler")
            .array("contracts", class("WeakIntroContract"))
            .array("introContracts", class("WeakIntroContract")),
        TestStruct::new("WeakIntroContract")
            .field("id", TestType::Guid)
            .field("debugName", TestType::String)
            .array("additionalPrerequisites", class(REQUIREMENT))
            .field("contractResults", class("WeakIntroResults")),
        TestStruct::new(REQUIREMENT).field("requiredCompletedContractTags", class("WeakIntroTags")),
        TestStruct::new("WeakIntroTags").array("tags", weak()),
        TestStruct::new("WeakIntroResults").array("contractResults", class(COMPLETION)),
        TestStruct::new(COMPLETION)
            .array("missionResults", class("WeakIntroOutcome"))
            .array("completionTags", class("WeakIntroGrant")),
        TestStruct::new("WeakIntroOutcome").field("value", TestType::Bool),
        TestStruct::new("WeakIntroGrant").field("tag", weak()),
    ];
    structures
        .into_iter()
        .fold(
            DataForgeBuilder::new().version(8),
            DataForgeBuilder::structure,
        )
        .record(tag_record(FIRST_TAG, "first"))
        .record(tag_record(SECOND_TAG, "second"))
        .record(generator())
        .build()
}

fn tag_record(name: &str, label: &str) -> TestRecord {
    TestRecord::new(
        TAG,
        "Tag",
        name,
        "libs/foundry/records/tagdatabase/tagdatabase.tagdatabase.xml",
    )
    .value("tagName", TestValue::text(label))
}

fn generator() -> TestRecord {
    let tags = TestInstance::new("WeakIntroTags").value(
        "tags",
        TestValue::Array(vec![TestValue::pointer(FIRST_TAG)]),
    );
    let prerequisite = TestInstance::new(REQUIREMENT)
        .value("requiredCompletedContractTags", TestValue::Struct(tags));
    let contract = TestInstance::new("WeakIntroContract")
        .value("id", TestValue::Guid(GATED))
        .value(
            "additionalPrerequisites",
            TestValue::Array(vec![TestValue::Struct(prerequisite)]),
        );
    let handler = TestInstance::new("WeakIntroHandler")
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        )
        .value("introContracts", TestValue::Array(vec![intro()]));
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.Weak",
        "libs/foundry/records/contracts/contractgenerator/weak_intro.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}

fn intro() -> TestValue {
    let success = TestInstance::new("WeakIntroOutcome").value("value", TestValue::Bool(true));
    let grant = TestInstance::new("WeakIntroGrant").value("tag", TestValue::pointer(SECOND_TAG));
    let result = TestInstance::new(COMPLETION)
        .value(
            "missionResults",
            TestValue::Array(vec![TestValue::Struct(success)]),
        )
        .value(
            "completionTags",
            TestValue::Array(vec![TestValue::Struct(grant)]),
        );
    let results = TestInstance::new("WeakIntroResults").value(
        "contractResults",
        TestValue::Array(vec![TestValue::Struct(result)]),
    );
    TestValue::Struct(
        TestInstance::new("WeakIntroContract")
            .value("id", TestValue::Guid(INTRO))
            .value("contractResults", TestValue::Struct(results)),
    )
}
