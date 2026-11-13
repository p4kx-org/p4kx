// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_export::Kind;
use p4kx_gui::CatalogExportStatus;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

use crate::cataloging::{TestResult, ended, opened};

const OWNER: &str = "MissionLocationTemplate";
const LOCATION: &str = "MissionLocationData";
const MODULE: &str = "SMissionLocationModule";
const SPAWN: &str = "AutoSpawnSettings";
const LIMIT: &str = "LocationMissionLimit";
const MANIFEST: &str = "SecurityNetworkManifest";
const MANIFEST_GUID: u128 = 0x1074_1020;

#[test]
fn normal_gui_catalog_publishes_disabled_location_configuration() -> TestResult {
    let (_directory, mut model, output) = opened(&archive())?;
    let path = output.join("locations.json");
    assert!(
        model.start_catalog(path.clone(), || {}),
        "the normal GUI worker starts"
    );
    let CatalogExportStatus::Finished(outcome) = ended(&mut model)? else {
        return Err("expected a normal GUI location catalog file".into());
    };
    assert!(
        outcome.summary().is_complete(),
        "the synthetic location and security definitions publish normally"
    );
    let summary = outcome
        .summary()
        .kind(Kind::MissionLocationTemplates)
        .ok_or("GUI mission location summary")?;
    assert_eq!(summary.count, 1, "one disabled location was published");
    let document: Value = serde_json::from_slice(&fs::read(path)?)?;
    verify_document(&document)
}

fn verify_document(document: &Value) -> TestResult {
    assert_eq!(
        document["schemaVersion"], 3,
        "the GUI worker emits catalog v3"
    );
    let row = document
        .pointer("/missionLocationTemplates/0")
        .ok_or("published location")?;
    assert_eq!(
        row["disabled"], true,
        "disabled source retains its configuration"
    );
    let configuration = row
        .pointer("/configuration/value")
        .ok_or("sourced configuration")?;
    for (path, expected) in [
        (
            "/missionModules/value/0/value/missionModule",
            json!("FixtureModule"),
        ),
        ("/missionModules/value/0/value/isPersistent", json!(false)),
        ("/missionLimits/value/0/value/maxMissionInstances", json!(0)),
        (
            "/missionLimits/value/1/value/maxMissionInstances",
            json!(-1),
        ),
        (
            "/autoSpawnSettings/value/0/value/excludeShipCrew",
            json!(true),
        ),
    ] {
        let cell = configuration
            .pointer(path)
            .ok_or("location configuration field")?;
        assert_eq!(cell["state"], "present", "authored value at {path}");
        assert_eq!(cell["value"], expected, "exact source value at {path}");
        assert_eq!(
            cell["containsEncryptedSource"], false,
            "plain DataForge at {path}"
        );
        assert_eq!(cell["sourceSetComplete"], true, "complete inputs at {path}");
        assert!(
            cell["sourceUsesRefs"]
                .as_array()
                .is_some_and(|uses| !uses.is_empty()),
            "original source-use reference at {path}"
        );
    }
    verify_security_link(configuration, document)
}

fn verify_security_link(configuration: &Value, document: &Value) -> TestResult {
    let target = configuration
        .pointer("/defaultSecurityNetworkManifest/value")
        .ok_or("authored default manifest link")?;
    let manifest = document
        .pointer("/definitions/securityNetworkManifests/rows/0")
        .ok_or("canonical security network definition")?;
    assert_eq!(target["status"], "present", "the security target resolves");
    assert!(
        target.pointer("/guid/value").is_some()
            && target.pointer("/guid/value") == manifest.pointer("/guid/value")
            && target.pointer("/targetRecordIndex/value") == manifest.pointer("/recordIndex/value"),
        "the written reference joins to the physical canonical definition"
    );
    Ok(())
}

fn archive() -> ArchiveBuilder {
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new(MANIFEST))
        .structure(
            TestStruct::new(MODULE)
                .field("missionModule", TestType::String)
                .field("isPersistent", TestType::Bool),
        )
        .structure(TestStruct::new(SPAWN).field("excludeShipCrew", TestType::Bool))
        .structure(TestStruct::new(LIMIT).field("maxMissionInstances", TestType::Int32))
        .structure(
            TestStruct::new(LOCATION)
                .field("disabled", TestType::Bool)
                .array("missionModules", class(MODULE))
                .array("missionLimits", class(LIMIT))
                .array("autoSpawnSettings", class(SPAWN))
                .field(
                    "defaultSecurityNetworkManifest",
                    TestType::ReferenceTo(MANIFEST.into()),
                ),
        )
        .structure(TestStruct::new(OWNER).field("locationData", class(LOCATION)))
        .record(TestRecord::new(
            MANIFEST_GUID,
            MANIFEST,
            "SecurityNetworkManifest.Fixture",
            "libs/foundry/records/synthetic/security/fixture.xml",
        ))
        .record(
            TestRecord::new(
                0x1074_1001,
                OWNER,
                "MissionLocationTemplate.Fixture",
                "libs/foundry/records/missiondata/pu_locations/fixture/location.xml",
            )
            .value("locationData", location()),
        );
    ArchiveBuilder::new().entry(TestEntry::stored("Data/Game2.dcb", database.build()))
}

fn location() -> TestValue {
    node(
        TestInstance::new(LOCATION)
            .value("disabled", TestValue::Bool(true))
            .value(
                "missionModules",
                TestValue::Array(vec![node(
                    TestInstance::new(MODULE)
                        .value("missionModule", TestValue::text("FixtureModule"))
                        .value("isPersistent", TestValue::Bool(false)),
                )]),
            )
            .value(
                "missionLimits",
                TestValue::Array(
                    [0, -1]
                        .into_iter()
                        .map(|maximum| {
                            node(
                                TestInstance::new(LIMIT)
                                    .value("maxMissionInstances", TestValue::Int32(maximum)),
                            )
                        })
                        .collect(),
                ),
            )
            .value(
                "autoSpawnSettings",
                TestValue::Array(vec![node(
                    TestInstance::new(SPAWN).value("excludeShipCrew", TestValue::Bool(true)),
                )]),
            )
            .value(
                "defaultSecurityNetworkManifest",
                TestValue::Reference(MANIFEST_GUID),
            ),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.into())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
