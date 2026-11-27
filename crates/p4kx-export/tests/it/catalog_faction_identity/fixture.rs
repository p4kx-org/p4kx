// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use super::{
    DESCRIPTION, FACTION, FIRST, MISSING, NAME, SECOND, SECURITY_FIRST, SECURITY_SECOND, TAG,
};
use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult};

const ARCHETYPE: u128 = 0x1085_0007;
const WEIGHT: &str = "EncounterPresetWeight";
const PRESET: &str = "EncounterPreset";

pub(super) fn export(
    filter: Option<&Filter>,
    weight: TestType,
    diagnostic: bool,
) -> TestResult<Value> {
    let (_directory, archive) = common::open(&database(weight), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    if diagnostic {
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    Ok(serde_json::from_slice(
        build_catalog(&archive, options.with_full_provenance())?.json(),
    )?)
}

pub(super) fn database(weight: TestType) -> DataForgeBuilder {
    let weight_values = if matches!(weight, TestType::Single) {
        [TestValue::Single(-0.0), TestValue::Single(1.25)]
    } else {
        [
            TestValue::text("Fixture invalid weight"),
            TestValue::text("Fixture other weight"),
        ]
    };
    let encounters = ["FixtureAlpha", "FixtureFuture"]
        .into_iter()
        .zip(weight_values)
        .map(|(preset, weight)| {
            TestValue::Struct(
                TestInstance::new(WEIGHT)
                    .value("encounterPreset", TestValue::text(preset))
                    .value("weight", weight),
            )
        })
        .collect();
    schema(weight)
        .record(record(FACTION, "Faction", "factions/shared"))
        .record(record(TAG, "Tag", "tags/original"))
        .record(record(
            SECURITY_FIRST,
            "SecurityNetworkManifest",
            "security/first",
        ))
        .record(record(
            SECURITY_SECOND,
            "SecurityNetworkManifest",
            "security/second",
        ))
        .record(identity(SECOND, "identities/second", SECURITY_SECOND, NAME))
        .record(
            identity(FIRST, "identities/first", SECURITY_FIRST, NAME)
                .value("encounterPresetWeights", TestValue::Array(encounters))
                .value(
                    "lootPoolThemeTags",
                    TestValue::Array(vec![
                        TestValue::Reference(TAG),
                        TestValue::Null,
                        TestValue::Reference(MISSING),
                    ]),
                ),
        )
        .record(record(ARCHETYPE, "NPCArchetype", "archetypes/owner").value(
            "identity",
            TestValue::Array(vec![
                    TestValue::Struct(TestInstance::new("NPCIdentityToArchetype")
                        .value("identity", TestValue::Reference(FIRST))),
                ]),
        ))
}

fn schema(weight: TestType) -> DataForgeBuilder {
    let reference = |name: &str| TestType::ReferenceTo(name.to_owned());
    let class = |name: &str| TestType::Class(name.to_owned());
    DataForgeBuilder::new()
        .version(8)
        .enumeration(PRESET, ["FixtureAlpha", "FixtureFuture"])
        .structure(TestStruct::new("Faction"))
        .structure(TestStruct::new("Tag"))
        .structure(TestStruct::new("SecurityNetworkManifest"))
        .structure(TestStruct::new("NPCClassPool"))
        .structure(
            TestStruct::new("NPCIdentityToArchetype")
                .field("identity", reference("FactionIdentity"))
                .array("npcPools", reference("NPCClassPool")),
        )
        .structure(
            TestStruct::new("NPCArchetype").array("identity", class("NPCIdentityToArchetype")),
        )
        .structure(
            TestStruct::new(WEIGHT)
                .field("encounterPreset", TestType::Enum(PRESET.to_owned()))
                .field("weight", weight),
        )
        .structure(
            TestStruct::new("FactionIdentity")
                .field("name", TestType::Locale)
                .field("description", TestType::Locale)
                .field("factionTag", reference("Tag"))
                .field("factionRef", reference("Faction"))
                .field("securityNetworkRef", reference("SecurityNetworkManifest"))
                .array("encounterPresetWeights", class(WEIGHT))
                .array("lootPoolThemeTags", reference("Tag")),
        )
}

pub(super) fn identity(id: u128, path: &str, security: u128, name: &str) -> TestRecord {
    record(id, "FactionIdentity", path)
        .value("name", TestValue::text(name))
        .value("description", TestValue::text(DESCRIPTION))
        .value("factionTag", TestValue::Reference(TAG))
        .value("factionRef", TestValue::Reference(FACTION))
        .value("securityNetworkRef", TestValue::Reference(security))
}

fn record(id: u128, family: &str, path: &str) -> TestRecord {
    TestRecord::new(
        id,
        family,
        format!("{family}.Fixture{id}"),
        format!("libs/foundry/records/faction_identity_fixture/{path}.xml"),
    )
}
