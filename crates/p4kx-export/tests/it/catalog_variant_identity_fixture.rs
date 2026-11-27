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
pub(super) const PATCH_PEERS: &str = "acas_patchpeer_";
pub(super) const PATCH_BASE_CLASS: &str = "ACAS_PatchPeer_Base";
pub(super) const PATCH_VARIANT_CLASS: &str = "ACAS_PatchPeer_Variant";
pub(super) const PATCH_OWNER_CLASS: &str = "ACAS_A_ExplicitPatch";
const PATCH_XML: &str =
    "Data/Scripts/Entities/Vehicles/Implementations/Xml/Modifications/explicit.xml";

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
    open_vehicles(
        &database,
        [
            TestEntry::stored(format!("Data/{DEFINITION_A}"), implementation(111, 50)),
            TestEntry::stored(format!("Data/{DEFINITION_B}"), implementation(222, 80)),
        ],
    )
}

pub(super) fn open_explicit_patch(
    patch: Option<&[u8]>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let database = schema()
        .record(record(BASE, PATCH_BASE_CLASS, DEFINITION_A))
        .record(record(VARIANT, PATCH_VARIANT_CLASS, DEFINITION_A))
        .record(modified_record(
            OTHER,
            PATCH_OWNER_CLASS,
            DEFINITION_A,
            "Explicit",
        ));
    let base = TestEntry::stored(
        format!("Data/{DEFINITION_A}"),
        br#"<Vehicle>
            <Parts><Part name="hull" class="Animated" mass="111" damageMax="50"/></Parts>
            <Modifications><Modification name="Explicit" patchFile="Modifications/explicit.xml"/></Modifications>
        </Vehicle>"#,
    );
    let patch = patch.map(|bytes| TestEntry::stored(PATCH_XML, bytes));
    open_vehicles(&database, std::iter::once(base).chain(patch))
}

fn open_vehicles(
    database: &DataForgeBuilder,
    entries: impl IntoIterator<Item = TestEntry>,
) -> TestResult<(tempfile::TempDir, Archive)> {
    let folder = tempfile::tempdir()?;
    let path = folder.path().join("fixture.p4k");
    let archive = ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .entry(TestEntry::stored(
            "Data/Libs/CosmoTools/LayerRecovery/LayerBackups.xml",
            b"<LayerBackups/>",
        ));
    entries
        .into_iter()
        .fold(archive, ArchiveBuilder::entry)
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
    modified_record(guid, class, definition, "")
}

fn modified_record(guid: u128, class: &str, definition: &str, modification: &str) -> TestRecord {
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
                .value("modification", TestValue::text(modification)),
        )]),
    )
}

fn implementation(mass: u32, health: u32) -> Vec<u8> {
    format!(
        r#"<Vehicle><Parts><Part name="hull" class="Animated" mass="{mass}" damageMax="{health}"/></Parts></Vehicle>"#
    )
    .into_bytes()
}
