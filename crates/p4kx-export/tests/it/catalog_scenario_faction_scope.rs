// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{ExportReferenceStatus, ExportSourceRole, Filter, Kind, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult, at, guid};

const FACTION: u128 = 0x0010_6681;
const REPUTATION: u128 = 0x0010_6682;
const ABSENT_REPUTATION: u128 = 0x0010_6683;
const SCENARIO: u128 = 0x0010_6684;
const NONFINITE_WORD: u32 = 0x7fc0_0666;

fn database(target: u128) -> DataForgeBuilder {
    let tier = "SScenarioProgressRewardsTiers";
    let faction_type = "Faction";
    let reputation_type = "FactionReputation";
    let scenario_type = "ScenarioProgress";
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(faction_type)
                .field("fixtureUnrelatedFloat", TestType::Single)
                .field(
                    "factionReputationRef",
                    TestType::ReferenceTo(reputation_type.to_owned()),
                ),
        )
        .structure(TestStruct::new(reputation_type))
        .structure(
            TestStruct::new(scenario_type)
                .array("factionRewardTiers", TestType::Class(tier.to_owned())),
        )
        .structure(
            TestStruct::new(tier).field("faction", TestType::ReferenceTo(faction_type.to_owned())),
        )
        .record(
            TestRecord::new(
                FACTION,
                faction_type,
                "Faction.Fixture",
                "libs/foundry/records/factions/faction_fixture_scope.xml",
            )
            .value(
                "fixtureUnrelatedFloat",
                TestValue::Single(f32::from_bits(NONFINITE_WORD)),
            )
            .value("factionReputationRef", TestValue::Reference(target)),
        )
        .record(TestRecord::new(
            REPUTATION,
            reputation_type,
            "FactionReputation.Fixture",
            "libs/foundry/records/factions/factionreputation/fixture.xml",
        ))
        .record(
            TestRecord::new(
                SCENARIO,
                scenario_type,
                "ScenarioProgress.Fixture",
                "libs/foundry/records/contracts/contractscenarios/fixture.xml",
            )
            .value(
                "factionRewardTiers",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(tier).value("faction", TestValue::Reference(FACTION)),
                )]),
            ),
        )
}

#[test]
fn scenario_faction_lookup_uses_only_the_observed_reputation_field() -> TestResult {
    let (_folder, archive) = fixture::open(&database(REPUTATION), DcbOrigin::Encrypted)?;
    let filter = Filter::new("contractscenarios");
    let mut options = fixture::options();
    options.filter = Some(&filter);
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = at(&catalog, "/contractScenarios/0/factionRewardTiers/0")?;
    assert_eq!(
        at(row, "/factionGuid")?,
        &json!(guid(FACTION)),
        "the original scenario target stays intact"
    );
    assert_eq!(
        at(row, "/factionReputationGuid")?,
        &json!(guid(REPUTATION)),
        "the actual consumed reputation link resolves"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        "complete",
        "the unrelated value does not enter scenario health"
    );
    Ok(())
}

#[test]
fn scoped_scenario_dependency_keeps_the_actual_dangling_reputation_diagnostic() -> TestResult {
    let (_folder, archive) = fixture::open(&database(ABSENT_REPUTATION), DcbOrigin::Encrypted)?;
    let filter = Filter::new("contractscenarios");
    let mut options = fixture::options();
    options.filter = Some(&filter);
    let built = build_catalog(&archive, &options).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = at(&catalog, "/contractScenarios/0/factionRewardTiers/0")?;
    assert_eq!(
        at(row, "/factionGuid")?,
        &json!(guid(FACTION)),
        "the original scenario target stays intact"
    );
    assert!(
        at(row, "/factionReputationGuid")?.is_null(),
        "a dangling reputation retains the scenario projection contract"
    );
    let references = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("original unresolved references")?;
    assert!(
        references.iter().any(|reference| {
            reference.get("kind") == Some(&json!(Kind::ContractScenarios))
                && reference.get("sourceRole") == Some(&json!(ExportSourceRole::Dependency))
                && reference.get("status") == Some(&json!(ExportReferenceStatus::Dangling))
                && reference.pointer("/targetGuid/value") == Some(&json!(guid(ABSENT_REPUTATION)))
                && reference.pointer("/targetGuid/containsEncryptedSource") == Some(&json!(true))
        }),
        "the actual observed dangling field remains attributed to the scenario dependency"
    );
    Ok(())
}

#[test]
fn a_faction_selected_alongside_its_scenario_retains_the_complete_original_fields() -> TestResult {
    let (_folder, archive) = fixture::open(&database(REPUTATION), DcbOrigin::Encrypted)?;
    let built = build_catalog(&archive, &fixture::options()).map_err(|error| error.to_string())?;
    let catalog: Value = serde_json::from_slice(built.json())?;
    let row = at(&catalog, "/contractScenarios/0/factionRewardTiers/0")?;
    let faction = at(&catalog, "/definitions/factions/rows/0")?;
    assert_eq!(
        at(row, "/factionGuid")?,
        at(faction, "/guid/value")?,
        "both scopes use the same physical Faction"
    );
    assert_eq!(
        at(
            faction,
            "/authoredFields/values/0/value/value/value/rawBits"
        )?,
        "7fc00666",
        "the selected Faction retains its separate original number proof"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        "complete",
        "whole and scoped field inspections coexist"
    );
    Ok(())
}
