// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{Kind, SCHEMA_VERSION};
use serde::Serialize;
use serde_json::{Map, Value, json};

use super::TestResult;
use crate::catalog_v3_consumer;

#[test]
fn text_census_binds_loose_planet_definitions_to_physical_entries() -> TestResult<()> {
    let kinds = [
        Kind::PlanetResources,
        Kind::Creatures,
        Kind::ClimateAssets,
        Kind::GeologyAssets,
        Kind::Ecosystems,
        Kind::GeologySets,
        Kind::GeologyTextureSets,
        Kind::Soils,
        Kind::SoilTextureSets,
        Kind::ShadingClasses,
    ];
    let mut definitions = Map::new();
    let mut sources = Vec::new();
    for (position, kind) in kinds.into_iter().enumerate() {
        let index = u32::try_from(position)?;
        let id = format!("fixture-entry-{index}");
        let path = format!(r"Data/Fixture\{kind}.json");
        let normalized = format!(r"data\fixture\{}.json", kind.name().to_ascii_lowercase());
        definitions.insert(
            kind.name().to_owned(),
            json!({"availability": "available", "rows": [{
                "identity": {
                    "inputRef": id,
                    "entryIndex": index,
                    "sourcePath": sourced("present", &path),
                    "normalizedPath": sourced("present", &normalized),
                },
                "guid": sourced("absent", Value::Null)
            }]}),
        );
        sources.push(json!({
            "id": id, "kind": "archiveEntry", "entryIndex": index,
            "path": sourced("present", &path)
        }));
    }
    let input = json!({
        "schemaVersion": SCHEMA_VERSION,
        "definitions": definitions,
        "provenance": {"sources": sources}
    });
    let catalog = project(&input)?;
    for kind in kinds {
        assert_eq!(
            catalog_v3_consumer::rows(&catalog, kind)?.len(),
            1,
            "{kind} binds its physical Entry independently of the nominal GUID",
        );
    }

    reject_a_foreign_ecosystem_entry(input)
}

fn reject_a_foreign_ecosystem_entry(mut input: Value) -> TestResult<()> {
    let index = input
        .pointer_mut("/definitions/ecosystems/rows/0/identity/entryIndex")
        .ok_or("projected ecosystem Entry index")?;
    *index = json!(100);
    let malformed = project(&input)?;
    let failure = catalog_v3_consumer::rows(&malformed, Kind::Ecosystems)
        .expect_err("a loose definition must match its physical Entry source");
    assert!(
        failure.to_string().contains("Entry position disagrees"),
        "reject a change to the Entry index even when the original path is preserved",
    );
    Ok(())
}

#[test]
fn text_census_checks_the_sourced_mission_scenario_header() -> TestResult<()> {
    const GUID: &str = "00000000-0000-0000-0000-000000000001";
    let input = json!({
        "schemaVersion": SCHEMA_VERSION,
        "definitions": {"missionScenarios": {"availability": "available", "rows": [{
            "guid": sourced("present", GUID),
            "recordName": sourced("present", "MissionScenario.Fixture"),
            "className": sourced("present", "fixture"),
            "recordPath": sourced("present", "libs/foundry/records/fixture.xml"),
            "recordIndex": sourced("present", 3),
            "sourceType": sourced("present", "MissionScenario"),
            "fileName": sourced("present", "libs/foundry/records/fixture.xml"),
            "metadataTag": sourced("absent", Value::Null),
            "authoredFields": {"roots": [], "values": []}
        }]}}
    });
    let catalog = project(&input)?;
    let rows = catalog_v3_consumer::rows(&catalog, Kind::MissionScenarios)?;
    let row = rows.first().ok_or("projected Mission Scenario")?;
    assert_eq!(
        catalog_v3_consumer::identity(row, Kind::MissionScenarios)?,
        GUID,
        "the nominal identity comes from the independently sourced Record GUID",
    );

    for (field, invalid) in [
        ("guid", json!(GUID)),
        ("recordIndex/value", json!(u64::from(u32::MAX) + 1)),
    ] {
        let mut broken = input.clone();
        let path = format!("/definitions/missionScenarios/rows/0/{field}");
        *broken
            .pointer_mut(&path)
            .ok_or("fixture Record header field")? = invalid;
        let malformed = project(&broken)?;
        assert!(
            catalog_v3_consumer::rows(&malformed, Kind::MissionScenarios).is_err(),
            "reject a malformed sourced {field}",
        );
    }
    Ok(())
}

fn project(catalog: &Value) -> TestResult<Value> {
    let encoded = serde_json::to_vec(catalog)?;
    Ok(super::catalog_paths().read(encoded.as_slice())?)
}

fn sourced(state: &str, value: impl Serialize) -> Value {
    json!({
        "state": state,
        "value": value,
        "containsEncryptedSource": false,
        "sourceSetComplete": true,
        "sourceUsesRefs": ["fixture-use"],
    })
}
