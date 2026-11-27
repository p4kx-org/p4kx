// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Language};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue, test_key,
};
use serde_json::{Value, json};

use super::ReducedError;
use super::blueprint_catalog;
use super::contract_scenarios::{self, Dependencies, PATH};
use super::factions;
use super::record_targets::RecordTargets;
use super::reputation_catalog;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::{CatalogLanguageSelection, CatalogQuerySession};

type TestResult = Result<(), Box<dyn std::error::Error>>;

const FACTION: &str = "Faction";
const REPUTATION: &str = "FactionReputation";
const POOL: &str = "BlueprintPoolRecord";
const SCENARIO: &str = "ScenarioProgress";
const GROUP: &str = "SScenarioProgressRewardsTiers";
const PROGRESSION: &str = "STierProgressions";
const TIER: &str = "STierReward";
const REPUTATION_GUID: u128 = 0x1177_1801;
const FACTION_GUID: u128 = 0x1177_1802;
const POOL_GUID: u128 = 0x1177_1803;
const SCENARIO_GUID: u128 = 0x1177_1804;
const UNKNOWN_GUID: u128 = 0x1177_1805;

#[test]
fn contract_scenarios_keep_selected_tiers_and_exact_physical_dependencies() -> TestResult {
    let scratch = tempfile::tempdir()?;
    let path = scratch.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()).encrypted())
        .write_to(&path)?;
    let archive = Archive::open_with_key(path, Some(test_key()))?;
    let session = CatalogQuerySession::open(
        &archive,
        CatalogLanguageSelection::Selected(&[Language::English, Language::German]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let mut texts = SelectedTexts::default();
    let reputation = reputation_catalog::collect(&parsed, scratch.path(), None, &mut texts)?;
    let factions = factions::collect(
        &parsed,
        &reputation.faction_targets,
        scratch.path(),
        None,
        &mut texts,
    )?;
    let blueprints = blueprint_catalog::collect(&parsed, scratch.path(), None, &mut texts)?;
    let dependencies = Dependencies {
        factions: &factions.targets,
        reputations: &reputation.faction_targets,
        pools: &blueprints.pool_targets,
    };
    let sequence = contract_scenarios::collect(&parsed, &dependencies, scratch.path(), None)?;
    let projected = serde_json::to_value(sequence)?;
    let original = projected
        .pointer("/0/factionRewardTiers/value/0/value")
        .ok_or("original faction slot")?;
    verify_tiers(original)?;
    let missing_pool = Dependencies {
        pools: &RecordTargets::default(),
        ..dependencies
    };
    assert!(
        matches!(
            contract_scenarios::collect(&parsed, &missing_pool, scratch.path(), None),
            Err(ReducedError::UnresolvedRecordLink {
                path: PATH,
                index: 2
            })
        ),
        "a physically resolved pool must exist in the selected definition table"
    );
    Ok(())
}

fn verify_tiers(group: &Value) -> TestResult {
    assert_eq!(
        group.pointer("/faction/value/targetRecordIndex/value"),
        Some(&json!(1)),
        "the source chooses the physical faction"
    );
    assert_eq!(
        group.pointer("/factionReputation/value/targetRecordIndex/value"),
        Some(&json!(0)),
        "the faction chooses the physical reputation"
    );
    assert_eq!(
        group.pointer("/pointsAsPercent/value"),
        Some(&json!(true)),
        "authored conversion flag"
    );
    let progression = group
        .pointer("/progressions/value/0/value")
        .ok_or("original progression")?;
    let reward = progression
        .pointer("/tiers/value/0/value")
        .ok_or("original reward tier")?;
    assert_eq!(
        reward.pointer("/minPoints/value"),
        Some(&json!({"type": "int32", "value": 12})),
        "authored threshold with its exact storage width"
    );
    let slots = reward
        .pointer("/blueprintPools/value")
        .and_then(Value::as_array)
        .ok_or("original pool slots")?;
    let [first, dangling, repeated] = slots.as_slice() else {
        return Err("all three ordered pool slots".into());
    };
    assert_eq!(
        first.pointer("/value/targetRecordIndex/value"),
        Some(&json!(2)),
        "pool closure uses the physical target"
    );
    assert_eq!(
        first.pointer("/value/guid/containsEncryptedSource"),
        Some(&json!(true)),
        "the original DataForge supplies the marker"
    );
    assert_eq!(
        first, repeated,
        "both occurrences of the pool reference survive"
    );
    assert_eq!(
        dangling.pointer("/value/status"),
        Some(&json!("dangling")),
        "the absent target remains a dangling reference"
    );
    assert!(
        progression.get("authoredFields").is_none(),
        "only the selected typed progression is delivered"
    );
    Ok(())
}

fn database() -> DataForgeBuilder {
    let progression = TestInstance::new(PROGRESSION).value("tierRewards", instances(reward()));
    let tiers = TestInstance::new(GROUP)
        .value("faction", TestValue::Reference(FACTION_GUID))
        .value("convertTiersPointsToPercent", TestValue::Bool(true))
        .value("tierProgressions", instances(progression));
    let data = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(REPUTATION))
        .structure(TestStruct::new(FACTION).field(
            "factionReputationRef",
            TestType::ReferenceTo(REPUTATION.to_owned()),
        ))
        .structure(TestStruct::new(POOL))
        .structure(
            TestStruct::new(SCENARIO)
                .array("factionRewardTiers", TestType::Class(GROUP.to_owned())),
        )
        .structure(
            TestStruct::new(GROUP)
                .field("faction", TestType::ReferenceTo(FACTION.to_owned()))
                .field("convertTiersPointsToPercent", TestType::Bool)
                .array("tierProgressions", TestType::Class(PROGRESSION.to_owned())),
        )
        .structure(
            TestStruct::new(PROGRESSION).array("tierRewards", TestType::Class(TIER.to_owned())),
        )
        .structure(
            TestStruct::new(TIER)
                .field("minPoints", TestType::Int32)
                .array("blueprintPool", TestType::ReferenceTo(POOL.to_owned())),
        );
    let roots = records(data);
    roots.record(
        TestRecord::new(
            SCENARIO_GUID,
            SCENARIO,
            "Fixture.Progress",
            "libs/foundry/records/contracts/contractscenarios/fixture.xml",
        )
        .value("factionRewardTiers", instances(tiers)),
    )
}

fn records(data: DataForgeBuilder) -> DataForgeBuilder {
    data.record(TestRecord::new(
        REPUTATION_GUID,
        REPUTATION,
        "FactionReputation.Fixture",
        "libs/foundry/records/factions/factionreputation/fixture.xml",
    ))
    .record(
        TestRecord::new(
            FACTION_GUID,
            FACTION,
            "Faction.Fixture",
            "libs/foundry/records/factions/faction_fixture.xml",
        )
        .value(
            "factionReputationRef",
            TestValue::Reference(REPUTATION_GUID),
        ),
    )
    .record(TestRecord::new(
        POOL_GUID,
        POOL,
        "BlueprintPoolRecord.Fixture",
        "libs/foundry/records/crafting/blueprintrewards/fixture.xml",
    ))
}

fn reward() -> TestInstance {
    TestInstance::new(TIER)
        .value("minPoints", TestValue::Int32(12))
        .value(
            "blueprintPool",
            TestValue::Array(
                [POOL_GUID, UNKNOWN_GUID, POOL_GUID]
                    .into_iter()
                    .map(TestValue::Reference)
                    .collect(),
            ),
        )
}

fn instances(value: TestInstance) -> TestValue {
    TestValue::Array(vec![TestValue::Struct(value)])
}
