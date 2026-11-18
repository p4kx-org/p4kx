// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

use crate::commodity_v3_fixture::TestResult;

pub(super) const BASE: u128 = 0x1110;
pub(super) const VARIANT: u128 = 0x1120;
const OTHER: u128 = 0x1130;
pub(super) const BASE_CLASS: &str = "ACAS_A_Base";
pub(super) const ALIAS_CLASS: &str = "ACAS_Z_Other";
pub(super) const VARIANT_CLASS: &str = "ACAS_M_Variant";
const DEFINITION_A: &str = "scripts/entities/vehicles/implementations/xml/fixture_a.xml";
const DEFINITION_B: &str = "scripts/entities/vehicles/implementations/xml/fixture_b.xml";

#[derive(Clone, Copy)]
pub(super) enum Identity {
    Shared,
    Distinct,
}

#[derive(Clone, Copy)]
pub(super) enum Key {
    Different,
    Same,
}

#[derive(Clone, Copy)]
pub(super) enum Order {
    BaseFirst,
    AliasFirst,
}

pub(super) fn open(
    identity: Identity,
    key: Key,
    order: Order,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let alias_guid = match identity {
        Identity::Shared => BASE,
        Identity::Distinct => OTHER,
    };
    let alias_definition = match key {
        Key::Different => DEFINITION_B,
        Key::Same => DEFINITION_A,
    };
    let mut records = [
        record(BASE, BASE_CLASS, DEFINITION_A),
        record(alias_guid, ALIAS_CLASS, alias_definition),
        record(VARIANT, VARIANT_CLASS, DEFINITION_A),
    ];
    if matches!(order, Order::AliasFirst) {
        records.swap(0, 1);
    }
    let database = records.into_iter().fold(schema(), DataForgeBuilder::record);
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .entry(TestEntry::stored(
            format!("Data/{DEFINITION_A}"),
            implementation(111, 50),
        ))
        .entry(TestEntry::stored(
            format!("Data/{DEFINITION_B}"),
            implementation(222, 80),
        ))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ))
        .write_to(&path)?;
    Ok((folder, Archive::open(path)?))
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("DataForgeComponentParams")
                .field("vehicleDefinition", TestType::String)
                .field("modification", TestType::String),
        )
}

fn record(guid: u128, class: &str, definition: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/spaceships/{class}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("VehicleComponentParams")
                .value("vehicleDefinition", TestValue::text(definition))
                .value("modification", TestValue::text("")),
        )]),
    )
}

fn implementation(mass: u32, health: u32) -> Vec<u8> {
    format!(
        r#"<Vehicle><Parts><Part name="hull" class="Animated" mass="{mass}" damageMax="{health}"/></Parts></Vehicle>"#
    )
    .into_bytes()
}
