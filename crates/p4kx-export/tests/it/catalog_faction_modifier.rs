// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Filter, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult, at, guid};
use crate::template_canonical_graph::Template;

const FACTION: u128 = 0x0010_6685;
const TEMPLATE: u128 = 0x0010_6686;
const MISSING: u128 = 0x0010_6687;
const MODIFIER: &str = "MissionModifier_FactionHostility";
const BASE: &str = "MissionModifier";

fn database(target: u128) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Faction"))
        .structure(
            TestStruct::new("ContractTemplate")
                .array("modifiers", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(MODIFIER)
                .parent(BASE)
                .field("faction", TestType::ReferenceTo("Faction".to_owned())),
        )
        .record(TestRecord::new(
            FACTION,
            "Faction",
            "Faction.Fixture",
            "libs/foundry/records/factions/faction_fixture_modifier.xml",
        ))
        .record(
            TestRecord::new(
                TEMPLATE,
                "ContractTemplate",
                "ContractTemplate.Fixture",
                "libs/foundry/records/contracts/contracttemplates/fixture_modifier.xml",
            )
            .value(
                "modifiers",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(MODIFIER).value("faction", TestValue::Reference(target)),
                )]),
            ),
        )
}

fn export(archive: &Archive, filter: Option<&Filter>) -> TestResult<Value> {
    let mut options = fixture::options();
    options.filter = filter;
    let built = build_catalog(archive, &options).map_err(|error| error.to_string())?;
    Ok(serde_json::from_slice(built.json())?)
}

fn modifier_reference(catalog: &Value) -> TestResult<&Value> {
    let graph = Template::new(catalog, &guid(TEMPLATE))?;
    let modifiers = crate::loot_v3_graph::array(graph.projection, "modifiers")?;
    let [modifier] = modifiers else {
        return Err("one original Faction modifier".into());
    };
    assert_eq!(
        at(modifier, "/kind/value")?,
        "factionHostility",
        "the existing modifier kind remains identifiable"
    );
    let body = graph.owner.follow(at(modifier, "/source")?)?;
    graph.field(body, "faction")
}

#[test]
fn original_faction_modifier_joins_without_rewriting_the_reference() -> TestResult {
    let (_folder, archive) = fixture::open(&database(FACTION), DcbOrigin::Encrypted)?;
    let catalog = export(&archive, None)?;
    let original = modifier_reference(&catalog)?;
    crate::template_fixture_fields::reference(original, &guid(FACTION))?;
    assert_eq!(
        at(original, "/value/value/value/guid/value")?,
        at(&catalog, "/definitions/factions/rows/0/guid/value")?,
        "the original modifier GUID joins to the Faction definition"
    );
    assert_eq!(
        at(original, "/value/containsEncryptedSource")?,
        true,
        "the original modifier Reference retains its immediate encrypted source"
    );
    let filter = Filter::new("contracttemplates");
    let filtered = export(&archive, Some(&filter))?;
    assert_eq!(
        at(&filtered, "/definitions/factions/rows")?,
        &json!([]),
        "the target is omitted by the row filter"
    );
    assert_eq!(
        modifier_reference(&filtered)?,
        original,
        "the original Reference remains unchanged after filtering the target"
    );
    Ok(())
}

#[test]
fn original_dangling_faction_modifier_keeps_its_stored_guid() -> TestResult {
    let (_folder, archive) = fixture::open(&database(MISSING), DcbOrigin::Encrypted)?;
    let catalog = export(&archive, None)?;
    let original = modifier_reference(&catalog)?;
    assert_eq!(
        at(original, "/originalError/type")?,
        "danglingReference",
        "the original failure state is explicit"
    );
    assert_eq!(
        at(original, "/originalError/guid/value")?,
        &json!(guid(MISSING)),
        "the absent target's GUID survives"
    );
    assert_eq!(
        at(original, "/originalError/guid/containsEncryptedSource")?,
        true,
        "the unresolved original GUID remains marked by its source"
    );
    Ok(())
}
