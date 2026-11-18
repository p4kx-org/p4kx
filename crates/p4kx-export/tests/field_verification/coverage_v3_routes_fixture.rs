// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::{Value, json};

const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const SHIELD: &str = "SCItemShieldGeneratorParams";
const DRIVE: &str = "SCItemQuantumDriveParams";
const MODE: &str = "SQuantumDriveParams";
const OTHER_MODE: &str = "FixtureUnmappedQuantumMode";

pub(super) fn catalog() -> super::super::Result<Value> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("route-fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database().build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let built = build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    assert!(
        built.summary().export_health().is_complete(),
        "the actual route fixture producer is healthy"
    );
    Ok(serde_json::from_slice(built.json())?)
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureItemType", ["Shield"])
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent(COMPONENT)
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(
            TestStruct::new(SHIELD)
                .parent(COMPONENT)
                .field("MaxShieldHealth", TestType::UInt32),
        )
        .structure(
            TestStruct::new(DRIVE)
                .parent(COMPONENT)
                .field("params", TestType::Class(MODE.to_owned()))
                .field("splineJumpParams", TestType::Class(MODE.to_owned())),
        )
        .structure(TestStruct::new(MODE).field("driveSpeed", TestType::UInt32))
        .record(owner())
}

fn owner() -> TestRecord {
    let attach = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("SItemDefinition").value("Type", TestValue::text("Shield")),
        ),
    );
    let shield = |health| {
        TestValue::Struct(
            TestInstance::new(SHIELD).value("MaxShieldHealth", TestValue::UInt32(health)),
        )
    };
    let drive = |standard, spline| {
        TestValue::Struct(
            TestInstance::new(DRIVE)
                .value("params", standard)
                .value("splineJumpParams", spline),
        )
    };
    TestRecord::new(
        1,
        ENTITY,
        "EntityClassDefinition.RouteFixture",
        "libs/foundry/records/entities/scitem/ships/route_fixture.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Struct(attach),
            shield(20),
            shield(30),
            drive(profile(1), profile(2)),
            drive(profile(3), profile(4)),
        ]),
    )
}

fn profile(speed: u32) -> TestValue {
    TestValue::Struct(TestInstance::new(MODE).value("driveSpeed", TestValue::UInt32(speed)))
}

fn pointer(kind: &str) -> TestType {
    TestType::StrongPointer(kind.to_owned())
}

/// Supplies an explicitly unsourced type label for the router's negative control.
pub(super) fn unmapped_standard(catalog: &mut Value) -> super::super::Result<()> {
    let source_type = catalog
        .pointer_mut("/configurations/quantumDrives/rows/1/standard/value/sourceType")
        .ok_or("actual second standard-mode type")?;
    *source_type = json!({
        "state": "present", "value": OTHER_MODE,
        "sourceSetComplete": false, "containsEncryptedSource": null, "sourceUsesRefs": []
    });
    Ok(())
}
