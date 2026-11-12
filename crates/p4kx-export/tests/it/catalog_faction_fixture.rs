// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const SECURITY: u128 = 0x0010_6601;
pub(super) const CIVIL: u128 = 0x0010_6602;
pub(super) const REPUTATION: u128 = 0x0010_6603;
pub(super) const MISSING: u128 = 0x0010_6604;
pub(super) const SCENARIO: u128 = 0x0010_6605;
pub(super) const SECURITY_NAME: &str = "@shared";
pub(super) const DESCRIPTION: &str = "Fixture faction description";
pub(super) const OUTSIDE: u128 = 0x0010_6606;
pub(super) const UNKNOWN_REACTION: &str = "FixtureUnrecognizedReaction";
pub(super) const FLAGS: [&str; 4] = [
    "ableToArrest",
    "policesLawfulTrespass",
    "policesCriminality",
    "noLegalRights",
];

const FACTION: &str = "Faction";
const REPUTATION_TYPE: &str = "FactionReputation";
const POLICY: &str = "FixtureFactionPolicy";
const REACTION: &str = "ReactionType";
const FACTION_TYPE: &str = "FactionType";
const OVERRIDE: &str = "FriendlyFireReactionOverride";
const SCENARIO_TYPE: &str = "ScenarioProgress";
const TIERS: &str = "SScenarioProgressRewardsTiers";

pub(super) fn complete() -> DataForgeBuilder {
    with_scenario(modern(3.25))
}

pub(super) fn unclassified_nonfinite() -> DataForgeBuilder {
    modern(f32::from_bits(0x7fc0_0666))
}

fn modern(unclassified_number: f32) -> DataForgeBuilder {
    let inherited = FLAGS
        .into_iter()
        .fold(TestStruct::new(POLICY), |policy, name| {
            policy.field(name, TestType::Bool)
        });
    base(modern_type())
        .enumeration(REACTION, ["FixturePassive", UNKNOWN_REACTION])
        .enumeration(FACTION_TYPE, ["FixtureCivil", "FixtureSecurity"])
        .structure(
            TestStruct::new(OVERRIDE)
                .field("reactionType", TestType::Enum(REACTION.to_owned()))
                .field("shouldAllowFriendlyFire", TestType::Bool),
        )
        .structure(inherited)
        .record(civil())
        .record(security(unclassified_number))
        .record(TestRecord::new(
            OUTSIDE,
            FACTION,
            "Faction.FixtureTemplate",
            "libs/foundry/records/synthetic/template_faction.xml",
        ))
}

fn modern_type() -> TestStruct {
    TestStruct::new(FACTION)
        .parent(POLICY)
        .field("name", TestType::Locale)
        .field("description", TestType::Locale)
        .field("defaultReaction", TestType::Enum(REACTION.to_owned()))
        .array("alliedFactions", reference(FACTION))
        .array("enemyFactions", reference(FACTION))
        .array(
            "friendlyFireBehaviorOverrides",
            TestType::Class(OVERRIDE.to_owned()),
        )
        .field("factionType", TestType::Enum(FACTION_TYPE.to_owned()))
        .field("factionReputationRef", reference(REPUTATION_TYPE))
        .field("fixtureAdditionalLegalFlag", TestType::Bool)
        .field("fixtureUnclassifiedNumber", TestType::Single)
}

fn civil() -> TestRecord {
    faction_record(CIVIL, "civil")
        .value("name", TestValue::text("@fixture_civil_name"))
        .value("description", TestValue::text(""))
        .value("defaultReaction", TestValue::text("FixturePassive"))
        .value("factionType", TestValue::text("FixtureCivil"))
        .value("factionReputationRef", TestValue::Reference(MISSING))
        .value("noLegalRights", TestValue::Bool(true))
}

fn security(unclassified_number: f32) -> TestRecord {
    faction_record(SECURITY, "security")
        .value("name", TestValue::text(SECURITY_NAME))
        .value("description", TestValue::text(SECURITY_NAME))
        .value("alliedFactions", links([CIVIL, MISSING, CIVIL]))
        .value("enemyFactions", links([OUTSIDE]))
        .value(
            "friendlyFireBehaviorOverrides",
            TestValue::Array(vec![
                override_for(UNKNOWN_REACTION, false),
                override_for("FixturePassive", true),
            ]),
        )
        .value("defaultReaction", TestValue::text(UNKNOWN_REACTION))
        .value("factionType", TestValue::text("FixtureSecurity"))
        .value("factionReputationRef", TestValue::Reference(REPUTATION))
        .value("ableToArrest", TestValue::Bool(true))
        .value("policesLawfulTrespass", TestValue::Bool(false))
        .value("policesCriminality", TestValue::Bool(true))
        .value("noLegalRights", TestValue::Bool(false))
        .value("fixtureAdditionalLegalFlag", TestValue::Bool(true))
        .value(
            "fixtureUnclassifiedNumber",
            TestValue::Single(unclassified_number),
        )
}

pub(super) fn legacy() -> DataForgeBuilder {
    let faction = TestStruct::new(FACTION)
        .field("name", TestType::String)
        .field("description", TestType::String)
        .field("factionReputationRef", reference(REPUTATION_TYPE));
    base(faction).record(
        faction_record(SECURITY, "legacy")
            .value("name", TestValue::text("Fixture short name"))
            .value("description", TestValue::text(DESCRIPTION))
            .value("factionReputationRef", TestValue::Null),
    )
}

fn base(faction: TestStruct) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(faction)
        .structure(TestStruct::new(REPUTATION_TYPE))
        .record(TestRecord::new(
            REPUTATION,
            REPUTATION_TYPE,
            "FactionReputation.Fixture",
            "libs/foundry/records/factions/factionreputation/fixture.xml",
        ))
}

fn with_scenario(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(SCENARIO_TYPE)
                .array("factionRewardTiers", TestType::Class(TIERS.to_owned())),
        )
        .structure(TestStruct::new(TIERS).field("faction", reference(FACTION)))
        .record(
            TestRecord::new(
                SCENARIO,
                SCENARIO_TYPE,
                "ScenarioProgress.Fixture",
                "libs/foundry/records/contracts/contractscenarios/fixture.xml",
            )
            .value(
                "factionRewardTiers",
                TestValue::Array(
                    [SECURITY, MISSING]
                        .into_iter()
                        .map(|guid| {
                            TestValue::Struct(
                                TestInstance::new(TIERS)
                                    .value("faction", TestValue::Reference(guid)),
                            )
                        })
                        .collect(),
                ),
            ),
        )
}

fn faction_record(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        FACTION,
        format!("Faction.{name}"),
        format!("libs/foundry/records/factions/faction_fixture_{name}.xml"),
    )
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn links<const N: usize>(guids: [u128; N]) -> TestValue {
    TestValue::Array(guids.into_iter().map(TestValue::Reference).collect())
}

fn override_for(label: &str, allow: bool) -> TestValue {
    TestValue::Struct(
        TestInstance::new(OVERRIDE)
            .value("reactionType", TestValue::text(label))
            .value("shouldAllowFriendlyFire", TestValue::Bool(allow)),
    )
}
