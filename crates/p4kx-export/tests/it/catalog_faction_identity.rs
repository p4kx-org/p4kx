// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogPublicationMode, Filter, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::{Value, json};

use crate::commodity_v3_fixture::{self as common, DcbOrigin, TestResult, at, guid};

const FACTION: u128 = 0x1085_0001;
const TAG: u128 = 0x1085_0002;
const SECURITY_FIRST: u128 = 0x1085_0003;
const SECURITY_SECOND: u128 = 0x1085_0004;
const FIRST: u128 = 0x1085_0005;
const SECOND: u128 = 0x1085_0006;
const ARCHETYPE: u128 = 0x1085_0007;
const MISSING: u128 = 0x1085_00ff;
const WEIGHT: &str = "EncounterPresetWeight";
const PRESET: &str = "EncounterPreset";
const NAME: &str = "FixtureFactionIdentityName";
const DESCRIPTION: &str = "FixtureFactionIdentityDescription";

#[test]
fn faction_identities_keep_their_own_links_weights_and_protected_text() -> TestResult {
    let catalog = export(None, TestType::Single, false)?;
    let identities = at(&catalog, "/definitions/factionIdentities/rows")?
        .as_array()
        .ok_or("physical identity rows")?;
    let [first, second] = identities.as_slice() else {
        return Err("both physical identities must be retained".into());
    };
    assert_eq!(
        at(first, "/guid/value")?,
        &json!(guid(FIRST)),
        "first identity"
    );
    assert_eq!(
        at(second, "/guid/value")?,
        &json!(guid(SECOND)),
        "second identity"
    );
    verify_identity_links(&catalog, first, second)?;
    verify_weights_and_theme_tags(first, second)?;
    let archetype = at(&catalog, "/definitions/npcArchetypes/rows/0")?;
    assert_eq!(
        at(
            archetype,
            "/identity/value/0/value/identity/value/guid/value"
        )?,
        &json!(guid(FIRST)),
        "existing NPC branch joins by physical identity"
    );
    let validator = jsonschema::validator_for(&p4kx_export::catalog_schema())?;
    if let Err(error) = validator.validate(&catalog) {
        return Err(format!("{}: {error}", error.instance_path()).into());
    }
    crate::commodity_v3_source_inventory::reference_closure(&catalog)
}

fn verify_identity_links(catalog: &Value, first: &Value, second: &Value) -> TestResult {
    for (row, security) in [(first, SECURITY_FIRST), (second, SECURITY_SECOND)] {
        assert_eq!(
            at(row, "/factionRef/value/guid/value")?,
            &json!(guid(FACTION)),
            "both owners retain the same Faction"
        );
        for (field, key) in [("name", NAME), ("description", DESCRIPTION)] {
            let cell = at(row, &format!("/{field}"))?;
            assert_eq!(at(cell, "/containsEncryptedSource")?, true, "{field}");
            let text = common::text_row(catalog, "gameTexts", at(cell, "/value")?)?;
            assert_eq!(at(text, "/key/value")?, key, "{field}");
        }
        assert_eq!(
            at(row, "/securityNetworkRef/value/guid/value")?,
            &json!(guid(security)),
            "each owner keeps its distinct security network"
        );
    }
    Ok(())
}

fn verify_weights_and_theme_tags(first: &Value, second: &Value) -> TestResult {
    for (index, preset, bits) in [
        (0, "FixtureAlpha", "80000000"),
        (1, "FixtureFuture", "3fa00000"),
    ] {
        let weight = at(
            first,
            &format!("/encounterPresetWeights/value/{index}/value"),
        )?;
        assert_eq!(
            at(weight, "/encounterPreset/value")?,
            preset,
            "preserve authored preset enum name and order"
        );
        assert_eq!(
            at(weight, "/weight/value/rawBits")?,
            bits,
            "exact authored Single"
        );
        assert_eq!(
            at(weight, "/weight/containsEncryptedSource")?,
            true,
            "weight is individually sourced"
        );
    }
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/0/value/guid/value")?,
        &json!(guid(TAG)),
        "original theme Tag"
    );
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/1/state")?,
        "null",
        "null list position"
    );
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/2/value/status")?,
        "dangling",
        "dangling tag remains in position"
    );
    assert_eq!(
        at(first, "/lootPoolThemeTags/value/2/value/guid/value")?,
        &json!(guid(MISSING)),
        "dangling target GUID"
    );
    assert_eq!(
        at(second, "/lootPoolThemeTags/value")?,
        &json!([]),
        "authored empty tag array"
    );
    assert_eq!(
        at(second, "/encounterPresetWeights/value")?,
        &json!([]),
        "authored empty weight array"
    );
    Ok(())
}

#[test]
fn filtered_faction_targets_leave_exact_identity_references() -> TestResult {
    let filter = Filter::new("*/faction_identity_fixture/identities/*");
    let catalog = export(Some(&filter), TestType::Single, false)?;
    assert_eq!(
        at(&catalog, "/definitions/factions/rows")?,
        &json!([]),
        "filter omits the Faction row"
    );
    let identity = at(&catalog, "/definitions/factionIdentities/rows/0")?;
    assert_eq!(
        at(identity, "/factionRef/value/guid/value")?,
        &json!(guid(FACTION)),
        "identity retains the exact source target"
    );
    let rows = at(&catalog, "/exportHealth/unresolvedReferences")?
        .as_array()
        .ok_or("unresolved catalog links")?;
    assert!(
        rows.iter().any(|row| {
            row["status"] == "excludedByFilter" && row["targetGuid"]["value"] == guid(FACTION)
        }),
        "filtered target gets explicit catalog status"
    );

    let filter = Filter::new("*/faction_identity_fixture/archetypes/*");
    let catalog = export(Some(&filter), TestType::Single, false)?;
    assert_eq!(
        at(&catalog, "/definitions/factionIdentities/rows")?,
        &json!([]),
        "identity row can be filtered independently"
    );
    let branch = at(
        &catalog,
        "/definitions/npcArchetypes/rows/0/identity/value/0/value/identity",
    )?;
    assert_eq!(
        at(branch, "/value/guid/value")?,
        &json!(guid(FIRST)),
        "NPC source link survives its filtered target"
    );
    Ok(())
}

#[test]
fn malformed_encounter_weight_retains_its_original_member_and_health() -> TestResult {
    let catalog = export(None, TestType::String, true)?;
    let weight = at(
        &catalog,
        "/definitions/factionIdentities/rows/0/encounterPresetWeights/value/0/value",
    )?;
    assert_eq!(
        at(weight, "/weight/state")?,
        "invalid",
        "mistyped weight is not converted to a number"
    );
    let values = at(weight, "/authoredFields/values")?
        .as_array()
        .ok_or("retained original fields")?;
    assert!(
        !values.is_empty(),
        "original malformed value stays in the source arena"
    );
    assert_eq!(
        at(&catalog, "/exportHealth/status")?,
        "incomplete",
        "invalid authored weight is visible to export health"
    );
    Ok(())
}

fn export(filter: Option<&Filter>, weight: TestType, diagnostic: bool) -> TestResult<Value> {
    let (_directory, archive) = common::open(&database(weight), DcbOrigin::Encrypted)?;
    let mut options = common::options();
    options.filter = filter;
    if diagnostic {
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    Ok(serde_json::from_slice(
        build_catalog(&archive, &options)?.json(),
    )?)
}

fn database(weight: TestType) -> DataForgeBuilder {
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
        .record(identity(SECOND, "identities/second", SECURITY_SECOND))
        .record(
            identity(FIRST, "identities/first", SECURITY_FIRST)
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

fn identity(id: u128, path: &str, security: u128) -> TestRecord {
    record(id, "FactionIdentity", path)
        .value("name", TestValue::text(NAME))
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
