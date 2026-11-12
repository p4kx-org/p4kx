// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with factions, reputation scopes, standings and contexts, mission types
//! and localities, and a contract scenario.

mod structures;

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestValue,
};

use structures::structures;

pub(crate) const FACTION: u128 = 0xe000_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const FACTION_BARE: u128 = 0xe000_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const FACTION_TEST: u128 = 0xe000_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const SCOPE: u128 = 0xe100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const STANDING_LOW: u128 = 0xe200_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const STANDING_HIGH: u128 = 0xe200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const STANDING_TEST: u128 = 0xe200_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const CONTEXT: u128 = 0xe300_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const MISSION_TYPE: u128 = 0xe400_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const LOCALITY: u128 = 0xe500_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PLAIN_A: u128 = 0xe600_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const PLAIN_B: u128 = 0xe600_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const MISSING: u128 = 0xe600_0000_0000_4000_8000_0000_0000_00ff;
pub(crate) const SCENARIO: u128 = 0xe700_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const LEGACY_FACTION: u128 = 0xe800_0000_0000_4000_8000_0000_0000_0001;

const RECORDS: &str = "libs/foundry/records/";

fn record(guid: u128, type_name: &str, name: &str, path: &str) -> TestRecord {
    TestRecord::new(
        guid,
        type_name,
        format!("{type_name}.{name}"),
        format!("{RECORDS}{path}.xml"),
    )
}

fn relation(standing: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("RelationStandingParams")
            .value("scope", TestValue::Reference(SCOPE))
            .value("standing", TestValue::Reference(standing)),
    )
}

fn property(name: &str, value: TestInstance) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SReputationContextBBPropertyParams")
            .value("name", TestValue::text(name))
            .value("dynamicProperty", TestValue::Struct(value)),
    )
}

fn faction(guid: u128, name: &str, bare: bool) -> TestRecord {
    let record = record(
        guid,
        "FactionReputation",
        &format!("FactionReputation_{name}"),
        &format!(
            "factions/factionreputation/factionreputation_{}",
            name.to_lowercase()
        ),
    )
    .value("displayName", TestValue::text("@faction_acme"));
    if bare {
        return record;
    }
    record
        .value("isNPC", TestValue::Bool(false))
        .value(
            "reputationContextPropertiesUI",
            TestValue::Reference(CONTEXT),
        )
        .value(
            "allies",
            TestValue::Array(vec![TestValue::Reference(FACTION_BARE)]),
        )
        .value("enemies", TestValue::Array(Vec::new()))
        .value("hostilityParams", relation(STANDING_LOW))
        .value("alliedParams", relation(STANDING_HIGH))
        .value(
            "propertiesBB",
            TestValue::Array(vec![
                property(
                    "entityDescription",
                    TestInstance::new("SBBDynamicPropertyLocString")
                        .value("value", TestValue::text("@faction_acme")),
                ),
                property(
                    "entityLawful",
                    TestInstance::new("SBBDynamicPropertyBool")
                        .value("value", TestValue::Bool(true)),
                ),
            ]),
        )
}

fn standing(guid: u128, name: &str, minimum: i64, gated: bool, hours: f32) -> TestRecord {
    record(
        guid,
        "SReputationStandingParams",
        name,
        &format!(
            "reputation/standings/faction/band_0/{}",
            name.to_lowercase()
        ),
    )
    .value("name", TestValue::text(name))
    .value(
        "description",
        TestValue::text("Synthetic standing description"),
    )
    .value("displayName", TestValue::text("@LOC_PLACEHOLDER"))
    .value("perkDescription", TestValue::text("@fixture_standing_perk"))
    .value("icon", TestValue::text("ui/fixture/standing.dds"))
    .value("missionBoardIcon", TestValue::text("ui/fixture/board.dds"))
    .value("minReputation", TestValue::Int64(minimum))
    .value("driftReputation", TestValue::Int64(-5))
    .value("driftTimeHours", TestValue::Single(hours))
    .value("gated", TestValue::Bool(gated))
}

fn scope() -> TestRecord {
    record(
        SCOPE,
        "SReputationScopeParams",
        "FactionReputationScope",
        "reputation/scopes/factionreputationscope",
    )
    .value("scopeName", TestValue::text("FactionReputation"))
    .value("displayName", TestValue::text("@scope_standing"))
    .value(
        "standingMap",
        TestValue::Struct(
            TestInstance::new("SReputationStandingMapParams")
                .value("reputationCeiling", TestValue::Int32(95_250))
                .value("initialReputation", TestValue::Int32(0))
                .value(
                    "standings",
                    TestValue::Array(vec![
                        TestValue::Reference(STANDING_LOW),
                        TestValue::Reference(STANDING_HIGH),
                    ]),
                ),
        ),
    )
}

fn scope_context(scope: u128) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SReputationScopeContextUI").value("scope", TestValue::Reference(scope)),
    )
}

fn context() -> TestRecord {
    record(
        CONTEXT,
        "SReputationContextUI",
        "ReputationContext_Transport",
        "reputation/contexts/reputationcontext_transport",
    )
    .value("primaryScopeContext", scope_context(SCOPE))
    .value(
        "scopeContextList",
        TestValue::Array(vec![scope_context(SCOPE), scope_context(PLAIN_A)]),
    )
}

fn tier(points: i32, badge: &str, pools: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("STierReward")
            .value("minPoints", TestValue::Int32(points))
            .value("badgeToAward", TestValue::text(badge))
            .value("globalPointsMultiplier", TestValue::Single(1.0))
            .value("blueprintPool", TestValue::Array(pools)),
    )
}

fn scenario() -> TestRecord {
    let progression = TestInstance::new("STierProgressions")
        .value("minMultiplierThreshold", TestValue::Single(1.0))
        .value(
            "completionType",
            TestValue::Struct(TestInstance::new("CompletionType_GlobalProgression")),
        )
        .value(
            "tierRewards",
            TestValue::Array(vec![
                tier(10_800, "ORS_Pistol", Vec::new()),
                tier(
                    18_000,
                    "None",
                    vec![TestValue::Reference(PLAIN_B), TestValue::Reference(MISSING)],
                ),
            ]),
        );
    record(
        SCENARIO,
        "ScenarioProgress",
        "ORS_ScenarioProgress",
        "contracts/contractscenarios/ors_scenarioprogress",
    )
    .value(
        "factionRewardTiers",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("SScenarioProgressRewardsTiers")
                .value("convertTiersPointsToPercent", TestValue::Bool(false))
                .value("faction", TestValue::Reference(LEGACY_FACTION))
                .value(
                    "tierProgressions",
                    TestValue::Array(vec![TestValue::Struct(progression)]),
                ),
        )]),
    )
}

fn database(high_drift_hours: f32) -> DataForgeBuilder {
    [
        faction(FACTION, "Acme", false),
        faction(FACTION_BARE, "Bare", true),
        faction(FACTION_TEST, "Test_Corp", true),
        scope(),
        standing(
            STANDING_LOW,
            "ReputationStanding_Faction_B0_00",
            0,
            false,
            -0.0,
        ),
        standing(
            STANDING_HIGH,
            "ReputationStanding_Faction_B0_01",
            4_500,
            true,
            high_drift_hours,
        ),
        standing(STANDING_TEST, "ReputationStanding_Test", 1, false, 3.0),
        context(),
        record(
            MISSION_TYPE,
            "MissionType",
            "missiontype.job",
            "missiontype/missiontype.job",
        )
        .value("LocalisedTypeName", TestValue::text("@type_job")),
        record(
            LOCALITY,
            "MissionLocality",
            "pyro",
            "missiondata/pu_missionlocality/pyro",
        )
        .value(
            "availableLocations",
            TestValue::Array(vec![
                TestValue::Reference(PLAIN_A),
                TestValue::Reference(MISSING),
                TestValue::Reference(PLAIN_B),
            ]),
        ),
        record(PLAIN_A, "Placeholder", "a", "misc/a"),
        record(PLAIN_B, "Placeholder", "b", "misc/b"),
        record(
            LEGACY_FACTION,
            "Faction",
            "Faction_Lawful_CrusaderIndustries",
            "factions/faction_lawful_crusaderindustries",
        )
        .value("factionReputationRef", TestValue::Reference(FACTION)),
        scenario(),
    ]
    .into_iter()
    .fold(
        structures(DataForgeBuilder::new().version(8)),
        DataForgeBuilder::record,
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with_drift(dir, 24.5)
}

/// Writes the fixture with a supplied source Single on the higher standing.
pub(super) fn write_archive_with_drift(
    dir: &Path,
    high_drift_hours: f32,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database(high_drift_hours).build()).encrypted())
        .entry(TestEntry::zstd(
            "Data\\Localization\\english\\global.ini",
            GlobalIniBuilder::new()
                .text("faction_acme", "Acme Emporium")
                .text("scope_standing", "Standing")
                .text("type_job", "Job")
                .text("fixture_standing_perk", "Synthetic perk")
                .build(),
        ))
        .write_to(&path)?;
    Ok(path)
}
