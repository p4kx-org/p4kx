// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Language;
use p4kx_testkit::{
    DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

use super::{records, structures};
use crate::health_origin_assertions::at;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

const GENERATOR: u128 = 0x1075_1001;
const ITEM: u128 = 0x1075_1002;
const MULTI: u128 = 0x1075_1003;
const MISSING: u128 = 0x1075_1004;
const TAG: u128 = 0x1075_1005;
const OTHER_TAG: u128 = 0x1075_1006;
const TAG_ROOT: u128 = 0x1075_1007;
const NESTED: u128 = 0x1075_1008;
const UNTRANSLATED: u128 = 0x1075_1009;

#[test]
fn mission_items_resolve_only_exact_source_candidates_and_variants() -> TestResult {
    let catalog = catalog()?;
    let row = title(&catalog, false)?;
    assert_eq!(
        at(row, "/translations/en/value")?,
        "One ready | [EMPTY] | [AMBIGUOUS] | [DEBUG] | [DANGLING] | [VARIANTS] | Chosen | [MISSINGVARIANT] | [UNTRANSLATED]",
        "only the single source target and variant can supply display text"
    );
    for (index, (token, reason)) in [
        ("Solo", None),
        ("Nested", None),
        ("Empty", Some("emptyCandidates")),
        ("Ambiguous", Some("ambiguousCandidates")),
        ("Debug", Some("runtimeQuery")),
        ("Dangling", Some("missingItem")),
        ("Variants", Some("ambiguousVariants")),
        ("Exact", None),
        ("MissingVariant", Some("missingVariant")),
        ("Untranslated", Some("missingTranslation")),
    ]
    .into_iter()
    .enumerate()
    {
        let audit = audit(row, token)?;
        let outcome = at(audit, "/outcome/value")?;
        assert_eq!(
            at(outcome, "/type")?,
            if reason.is_some() {
                "unresolved"
            } else {
                "staticPreview"
            },
            "audit decision for synthetic token {index}"
        );
        if let Some(reason) = reason {
            assert_eq!(at(outcome, "/reason")?, reason);
        }
        assert_eq!(at(audit, "/name/containsEncryptedSource")?, false);
        assert_eq!(at(audit, "/outcome/containsEncryptedSource")?, true);
    }
    assert_eq!(at(audit(row, "Exact")?, "/variant/value")?, "Chosen");
    assert_eq!(at(audit(row, "Solo")?, "/variant/value")?, &Value::Null);
    Ok(())
}

#[test]
fn mission_item_higher_layer_and_recursive_tokens_keep_their_original_brackets() -> TestResult {
    let catalog = catalog()?;
    let parent = title(&catalog, false)?;
    let child = title(&catalog, true)?;
    assert_eq!(at(child, "/translations/en/value")?, "[SOLO] / [RECURSIVE]");
    assert_eq!(
        at(audit(child, "Solo")?, "/outcome/value/reason")?,
        "emptyCandidates",
        "the sub-contract token shadows its parent's static input"
    );
    assert!(parent.pointer("/itemTokens/de").is_none());
    Ok(())
}

#[test]
fn generator_mission_item_inputs_follow_the_same_original_source_arena() -> TestResult {
    let catalog = catalog()?;
    let rows = at(&catalog, "/definitions/contractGenerators/rows")?
        .as_array()
        .ok_or("Generator definitions")?;
    let [generator] = rows.as_slice() else {
        return Err("one physical Generator".into());
    };
    let owner = crate::flow_owner_graph::Owner::complete(generator)?;
    let properties = at(generator, "/missionItemProperties")?
        .as_array()
        .ok_or("sourced Generator MissionItem interpretation")?;
    let [first, .., sub] = properties.as_slice() else {
        return Err("expected contract and child properties".into());
    };
    assert_eq!(at(first, "/level")?, "contract");
    assert_eq!(at(sub, "/level")?, "subContract");
    let selected = owner.follow(at(first, "/source")?)?;
    let maximum = at(first, "/missionItem/maxItemsToFind/field")?;
    let maximum = owner.field(maximum)?;
    let bounds = owner.follow(at(maximum, "/value")?)?;
    assert_eq!(at(bounds, "/value/value/value/value")?, 1);
    assert_eq!(at(bounds, "/value/containsEncryptedSource")?, true);
    let debug = owner.field(at(first, "/missionItem/debugForceChosenItem/field")?)?;
    let debug = owner.follow(at(debug, "/value")?)?;
    assert_eq!(at(debug, "/value/state")?, "null");
    let candidates = at(first, "/conditions/0/specificItems/candidates")?
        .as_array()
        .ok_or("specific candidates")?;
    let [candidate] = candidates.as_slice() else {
        return Err("one authored candidate".into());
    };
    let candidate = owner.follow(candidate)?;
    assert_eq!(
        at(candidate, "/value/value/value/guid/value")?.as_str(),
        Some(crate::health_origin_fixture::guid(ITEM).as_str())
    );
    assert_eq!(
        at(selected, "/value/value/sourceType/value")?,
        "MissionPropertyValue_MissionItem"
    );
    assert_eq!(
        at(
            owner.follow(at(first, "/propertySet")?)?,
            "/value/value/type"
        )?,
        "array"
    );
    assert_eq!(
        at(owner.follow(at(first, "/property")?)?, "/value/value/type")?,
        "struct"
    );
    Ok(())
}

fn catalog() -> TestResult<Value> {
    let builder = structures::structures(
        DataForgeBuilder::new()
            .version(8)
            .enumeration("ECurrency", ["UEC"]),
    )
    .structure(
        TestStruct::new("MissionPropertyValue_MissionItem")
            .parent("MissionPropertyValue")
            .field("minItemsToFind", TestType::Int32)
            .field("maxItemsToFind", TestType::Int32)
            .array(
                "matchConditions",
                TestType::StrongPointer("DataSetMatchCondition".to_owned()),
            )
            .field("debugForceChosenItem", reference_type("MissionItem")),
    )
    .structure(
        TestStruct::new("DataSetMatchCondition_SpecificItemsDef")
            .parent("DataSetMatchCondition")
            .array("items", reference_type("MissionItem")),
    )
    .structure(TestStruct::new("MissionItem").field(
        "stringVariants",
        TestType::Class("MissionStringVariants".to_owned()),
    ))
    .record(generator())
    .record(item(ITEM, &[(TAG, "@first")]))
    .record(item(MULTI, &[(TAG, "@chosen"), (OTHER_TAG, "@other")]))
    .record(item(NESTED, &[(TAG, "@nested")]))
    .record(item(UNTRANSLATED, &[(TAG, "@absent_translation")]))
    .record(tag(TAG, "Chosen"))
    .record(tag(OTHER_TAG, "Other"))
    .record(
        TestRecord::new(
            TAG_ROOT,
            "TagDatabase",
            "TagDatabase.Synthetic",
            "synthetic/tags.xml",
        )
        .value("tags", array(vec![reference(TAG), reference(OTHER_TAG)])),
    );
    let english = GlobalIniBuilder::new()
        .text(
            "token_parent",
            "~mission(Solo) | ~mission(Empty) | ~mission(Ambiguous) | ~mission(Debug) | ~mission(Dangling) | ~mission(Variants) | ~mission(Exact|Chosen) | ~mission(MissingVariant|Unknown) | ~mission(Untranslated)",
        )
        .text("token_child", "~mission(Solo) / ~mission(Recursive)")
        .text("first", "One ~mission(Nested)")
        .text("nested", "ready")
        .text("chosen", "Chosen")
        .text("other", "Other")
        .text("recursive", "~mission(Recursive)");
    let languages = vec![TestEntry::stored(
        Language::English.global_ini_path(),
        english.build(),
    )];
    let fixture = Fixture::open(&builder.build(), Input::Encrypted, languages)?;
    let (_, catalog) = fixture.diagnostic()?;
    Ok(catalog)
}

fn generator() -> TestRecord {
    let child = TestInstance::new("SubContract")
        .value("id", TestValue::Guid(GENERATOR + 1))
        .value(
            "paramOverrides",
            params(
                "@token_child",
                vec![
                    property("Solo", &[], None),
                    records::property("Recursive", records::string_hash("@recursive")),
                ],
            ),
        );
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(GENERATOR + 2))
        .value(
            "paramOverrides",
            params("@token_parent", parent_properties()),
        )
        .value("subContracts", array(vec![object(child)]));
    let handler = TestInstance::new("ContractGeneratorHandler_List")
        .value("contracts", array(vec![object(contract)]));
    TestRecord::new(
        GENERATOR,
        "ContractGenerator",
        "ContractGenerator.SyntheticTokens",
        "libs/foundry/records/contracts/contractgenerator/synthetic_tokens.xml",
    )
    .value("generators", array(vec![object(handler)]))
}

fn parent_properties() -> Vec<TestValue> {
    vec![
        property("Solo", &[ITEM], None),
        property("Nested", &[NESTED], None),
        property("Empty", &[], None),
        property("Ambiguous", &[ITEM, MULTI], None),
        property("Debug", &[ITEM], Some(MULTI)),
        property("Dangling", &[MISSING], None),
        property("Variants", &[MULTI], None),
        property("Exact", &[MULTI], None),
        property("MissingVariant", &[MULTI], None),
        property("Untranslated", &[UNTRANSLATED], None),
    ]
}

fn params(title: &str, properties: Vec<TestValue>) -> TestValue {
    let title = TestInstance::new("ContractStringParam")
        .value("param", TestValue::text("Title"))
        .value("value", TestValue::text(title));
    object(
        TestInstance::new("ContractParamOverrides")
            .value("stringParamOverrides", array(vec![object(title)]))
            .value("propertyOverrides", array(properties)),
    )
}

fn property(token: &str, items: &[u128], debug: Option<u128>) -> TestValue {
    let condition = TestInstance::new("DataSetMatchCondition_SpecificItemsDef").value(
        "items",
        array(items.iter().copied().map(reference).collect()),
    );
    let value = TestInstance::new("MissionPropertyValue_MissionItem")
        .value("minItemsToFind", TestValue::Int32(1))
        .value("maxItemsToFind", TestValue::Int32(1))
        .value("matchConditions", array(vec![object(condition)]))
        .value(
            "debugForceChosenItem",
            debug.map_or(TestValue::Null, reference),
        );
    records::property(token, object(value))
}

fn item(guid: u128, variants: &[(u128, &str)]) -> TestRecord {
    let variants = variants
        .iter()
        .map(|&(tag, key)| {
            object(
                TestInstance::new("MissionStringVariant")
                    .value("tag", reference(tag))
                    .value("string", TestValue::text(key)),
            )
        })
        .collect();
    TestRecord::new(
        guid,
        "MissionItem",
        "MissionItem.Synthetic",
        format!("synthetic/items/{guid}.xml"),
    )
    .value(
        "stringVariants",
        object(TestInstance::new("MissionStringVariants").value("variants", array(variants))),
    )
}

fn tag(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(guid, "Tag", format!("Tag.{name}"), "synthetic/tags.xml")
        .value("tagName", TestValue::text(name))
}

fn title(catalog: &Value, child: bool) -> TestResult<&Value> {
    let rows = at(catalog, "/projections/missionTitles/rows")?
        .as_array()
        .ok_or("canonical title rows")?;
    rows.iter()
        .find(|row| {
            row.pointer("/contract/subContract")
                .is_some_and(|value| value.is_null() != child)
        })
        .and_then(|row| row.get("title"))
        .ok_or_else(|| "selected physical contract title".into())
}

fn audit<'catalog>(row: &'catalog Value, name: &str) -> TestResult<&'catalog Value> {
    at(row, "/itemTokens/en")?
        .as_array()
        .and_then(|rows| {
            rows.iter()
                .find(|row| row.pointer("/name/value").and_then(Value::as_str) == Some(name))
        })
        .ok_or_else(|| "the authored token has an audit row".into())
}

fn reference_type(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

const fn object(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}

const fn reference(guid: u128) -> TestValue {
    TestValue::Reference(guid)
}

const fn array(values: Vec<TestValue>) -> TestValue {
    TestValue::Array(values)
}
