// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::CatalogOptions;
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

const SHIELD: u128 = 0x1111_0000_0000_4000_8000_0000_0000_0001;
const WEAPON: u128 = 0x1111_0000_0000_4000_8000_0000_0000_0002;
const PLAIN: u128 = 0x1111_0000_0000_4000_8000_0000_0000_0003;
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const ITEM: &str = "SItemDefinition";
const SHIELD_PARAMS: &str = "SCItemShieldGeneratorParams";
const WEAPON_PARAMS: &str = "SCItemWeaponComponentParams";
const ACTION: &str = "SWeaponActionParams";
const RAPID: &str = "SWeaponActionFireRapidParams";
const CHARGED: &str = "SWeaponActionFireChargedParams";

pub(super) fn catalog() -> super::super::Result<Value> {
    build(&database())
}

pub(super) fn catalog_with_alias() -> super::super::Result<Value> {
    build(&database().record(item(SHIELD, "audit_shield_alias", "Shield", [shield()])))
}

fn build(database: &DataForgeBuilder) -> super::super::Result<Value> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database.build()))
        .write_to(&path)?;
    let archive = Archive::open(path)?;
    let built = p4kx_export::build_catalog(&archive, &CatalogOptions::new(UNIX_EPOCH))?;
    Ok(serde_json::from_slice(built.json())?)
}

fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("EItemType", ["Shield", "WeaponGun", "Misc"])
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", TestType::Class(ITEM.to_owned())),
        )
        .structure(TestStruct::new(ITEM).field("Type", TestType::Enum("EItemType".to_owned())))
        .structure(
            TestStruct::new(SHIELD_PARAMS)
                .parent(COMPONENT)
                .field("MaxShieldHealth", TestType::Single),
        )
        .structure(
            TestStruct::new(WEAPON_PARAMS)
                .parent(COMPONENT)
                .array("fireActions", pointer(ACTION)),
        )
        .structure(TestStruct::new(ACTION))
        .structure(
            TestStruct::new(RAPID)
                .parent(ACTION)
                .field("fireRate", TestType::Int32),
        )
        .structure(
            TestStruct::new(CHARGED)
                .parent(ACTION)
                .field("weaponAction", pointer(ACTION)),
        )
        .record(item(SHIELD, "audit_shield", "Shield", [shield()]))
        .record(item(WEAPON, "audit_weapon", "WeaponGun", [weapon()]))
        .record(item(PLAIN, "audit_plain", "Misc", []))
}

fn item(
    id: u128,
    name: &str,
    kind: &str,
    components: impl IntoIterator<Item = TestValue>,
) -> TestRecord {
    let definition = TestInstance::new(ITEM).value("Type", TestValue::text(kind));
    let attach = TestValue::Struct(
        TestInstance::new(ATTACHABLE).value("AttachDef", TestValue::Struct(definition)),
    );
    let components = std::iter::once(attach).chain(components).collect();
    TestRecord::new(
        id,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/scitem/ships/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn shield() -> TestValue {
    TestValue::Struct(
        TestInstance::new(SHIELD_PARAMS).value("MaxShieldHealth", TestValue::Single(20.0)),
    )
}

fn weapon() -> TestValue {
    let charged = TestValue::Struct(TestInstance::new(CHARGED).value("weaponAction", rapid(120)));
    TestValue::Struct(TestInstance::new(WEAPON_PARAMS).value(
        "fireActions",
        TestValue::Array(vec![TestValue::Null, rapid(60), charged]),
    ))
}

fn rapid(rate: i32) -> TestValue {
    TestValue::Struct(TestInstance::new(RAPID).value("fireRate", TestValue::Int32(rate)))
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
