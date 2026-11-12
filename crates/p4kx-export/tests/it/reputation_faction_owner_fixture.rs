// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const MAIN: u128 = 0x1073_0101;
pub(super) const ALLY: u128 = 0x1073_0102;
pub(super) const MISSING: u128 = 0x1073_0103;
pub(super) const SCOPE: u128 = 0x1073_0104;
pub(super) const STANDING: u128 = 0x1073_0105;
pub(super) const CONTEXT: u128 = 0x1073_0106;
pub(super) const PERK_LIST: u128 = 0x1073_0107;
pub(super) const FACTION: u128 = 0x1073_0108;
pub(super) const GEID: u64 = u64::MAX - 5;
pub(super) const LITERAL: &str = "@fixture_original_literal";
pub(super) const BACKGROUND: &str = "@fixture_background_literal";
pub(super) const FLOAT_BITS: u32 = 0x8000_0000;
pub(super) const RAW_BITS: u32 = 0x7fc0_1073;
pub(super) const UNKNOWN: &str = "FixtureUnknownDynamicProperty";
pub(super) const TRIGGER: &str = "FixtureNewSandboxTrigger";

const OWNER: &str = "FactionReputation";
const SCOPE_TYPE: &str = "SReputationScopeParams";
const STANDING_TYPE: &str = "SReputationStandingParams";
const CONTEXT_TYPE: &str = "SReputationContextUI";
const PERK_LIST_TYPE: &str = "SPerkReputationListParams";
const RELATION: &str = "RelationStandingParams";
const MARKER: &str = "RelationMarkerParams";
const PROPERTY: &str = "SReputationContextBBPropertyParams";
const BASE: &str = "SBBDynamicPropertyBase";
const BOOLEAN: &str = "SBBDynamicPropertyBool";
const FLOAT: &str = "SBBDynamicPropertyFloat";
const INTEGER: &str = "SBBDynamicPropertyInt";
const LOCALE: &str = "SBBDynamicPropertyLocString";
const STRING: &str = "SBBDynamicPropertyString";
const TRIGGER_BASE: &str = "SandboxTriggerBaseDef";

pub(super) fn owner(legacy: bool) -> DataForgeBuilder {
    let text = if legacy {
        TestType::String
    } else {
        TestType::Locale
    };
    let mut builder = declarations(text);
    for (id, ty, path) in [
        (SCOPE, SCOPE_TYPE, "reputation/scopes/fixture_owner_scope"),
        (
            STANDING,
            STANDING_TYPE,
            "reputation/standings/fixture_owner_standing",
        ),
        (
            CONTEXT,
            CONTEXT_TYPE,
            "reputation/contexts/fixture_owner_context",
        ),
        (
            PERK_LIST,
            PERK_LIST_TYPE,
            "reputation/perks/fixture_owner_perks",
        ),
    ] {
        builder = builder
            .structure(TestStruct::new(ty))
            .record(record(id, ty, path));
    }
    builder
        .record(
            record(0x1073_0109, "MissionType", "missiontype/fixture_owner")
                .value("LocalisedTypeName", TestValue::text("@shared")),
        )
        .record(owner_value(
            record(MAIN, OWNER, "factions/factionreputation/fixture_main"),
            legacy,
        ))
        .record(record(
            ALLY,
            OWNER,
            "factions/factionreputation/fixture_ally",
        ))
        .record(
            record(FACTION, "Faction", "factions/faction_fixture_join")
                .value("factionReputationRef", TestValue::Reference(MAIN)),
        )
}

fn declarations(text: TestType) -> DataForgeBuilder {
    let mut builder = DataForgeBuilder::new()
        .version(8)
        .structure(owner_declaration(text))
        .structure(TestStruct::new("MissionType").field("LocalisedTypeName", TestType::Locale))
        .structure(relation_declaration())
        .structure(
            TestStruct::new(MARKER)
                .field("description", TestType::Locale)
                .field("icon", TestType::String),
        )
        .structure(
            TestStruct::new(PROPERTY)
                .field("name", TestType::String)
                .field("dynamicProperty", pointer(BASE)),
        )
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(TRIGGER_BASE))
        .structure(
            TestStruct::new(TRIGGER)
                .parent(TRIGGER_BASE)
                .field("fixtureOn", TestType::Bool),
        )
        .structure(TestStruct::new("Faction").field("factionReputationRef", reference(OWNER)));
    for (name, storage) in [
        (BOOLEAN, TestType::Bool),
        (FLOAT, TestType::Single),
        (INTEGER, TestType::Int32),
        (LOCALE, TestType::Locale),
        (STRING, TestType::String),
        (UNKNOWN, TestType::Single),
    ] {
        builder = builder.structure(TestStruct::new(name).parent(BASE).field("value", storage));
    }
    builder
}

fn owner_declaration(text: TestType) -> TestStruct {
    TestStruct::new(OWNER)
        .field("name", TestType::String)
        .field("displayName", text.clone())
        .field("description", text)
        .field("background", TestType::String)
        .field("GEID", TestType::UInt64)
        .field("logo", TestType::String)
        .field("missionBoardLogo", TestType::String)
        .field("guildBanner", TestType::String)
        .field("hideInDelpihApp", TestType::Bool)
        .field("isNPC", TestType::Bool)
        .field("reputationContextPropertiesUI", reference(CONTEXT_TYPE))
        .array("allies", reference(OWNER))
        .array("enemies", reference(OWNER))
        .field("hostilityParams", class(RELATION))
        .field("alliedParams", class(RELATION))
        .array("propertiesBB", class(PROPERTY))
        .array("perkReputationRewardList", reference(PERK_LIST_TYPE))
        .array("sandboxTriggers", pointer(TRIGGER_BASE))
}

fn relation_declaration() -> TestStruct {
    TestStruct::new(RELATION)
        .field("scope", reference(SCOPE_TYPE))
        .field("standing", reference(STANDING_TYPE))
        .field("markerParams", class(MARKER))
}

fn owner_value(record: TestRecord, legacy: bool) -> TestRecord {
    let description = if legacy { LITERAL } else { "@own_description" };
    record
        .value("name", TestValue::text("fixture_reputation_owner"))
        .value("displayName", TestValue::text("@shared"))
        .value("description", TestValue::text(description))
        .value("background", TestValue::text(BACKGROUND))
        .value("GEID", TestValue::UInt64(GEID))
        .value("logo", TestValue::text("textures/fixture/logo.dds"))
        .value(
            "missionBoardLogo",
            TestValue::text("textures/fixture/board.dds"),
        )
        .value(
            "guildBanner",
            TestValue::text("textures/fixture/banner.dds"),
        )
        .value("hideInDelpihApp", TestValue::Bool(false))
        .value("isNPC", TestValue::Bool(true))
        .value(
            "reputationContextPropertiesUI",
            TestValue::Reference(CONTEXT),
        )
        .value("allies", references([ALLY]))
        .value("enemies", references([MISSING, ALLY]))
        .value(
            "hostilityParams",
            TestValue::Struct(
                TestInstance::new(RELATION)
                    .value("scope", TestValue::Reference(SCOPE))
                    .value("standing", TestValue::Reference(STANDING))
                    .value(
                        "markerParams",
                        TestValue::Struct(
                            TestInstance::new(MARKER)
                                .value("description", TestValue::text("@own_description"))
                                .value("icon", TestValue::text("textures/fixture/relation.dds")),
                        ),
                    ),
            ),
        )
        .value("propertiesBB", TestValue::Array(properties()))
        .value("perkReputationRewardList", references([PERK_LIST]))
        .value(
            "sandboxTriggers",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new(TRIGGER).value("fixtureOn", TestValue::Bool(true)),
            )]),
        )
}

fn properties() -> Vec<TestValue> {
    [
        ("entityLawful", BOOLEAN, TestValue::Bool(false)),
        (
            "fixtureFloat",
            FLOAT,
            TestValue::Single(f32::from_bits(FLOAT_BITS)),
        ),
        ("fixtureInt", INTEGER, TestValue::Int32(-8)),
        ("fixtureLiteral", STRING, TestValue::text(LITERAL)),
        ("fixtureLocalized", LOCALE, TestValue::text("@shared")),
        (
            "fixtureUnknown",
            UNKNOWN,
            TestValue::Single(f32::from_bits(RAW_BITS)),
        ),
    ]
    .into_iter()
    .map(|(name, ty, value)| {
        TestValue::Struct(
            TestInstance::new(PROPERTY)
                .value("name", TestValue::text(name))
                .value(
                    "dynamicProperty",
                    TestValue::Struct(TestInstance::new(ty).value("value", value)),
                ),
        )
    })
    .collect()
}

fn references<const N: usize>(values: [u128; N]) -> TestValue {
    TestValue::Array(values.into_iter().map(TestValue::Reference).collect())
}

fn record(id: u128, ty: &str, path: &str) -> TestRecord {
    TestRecord::new(
        id,
        ty,
        format!("{ty}.Fixture"),
        format!("libs/foundry/records/{path}.xml"),
    )
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}
