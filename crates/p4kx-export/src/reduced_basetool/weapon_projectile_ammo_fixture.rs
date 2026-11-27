// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const PRIMARY: u128 = 0x0117_7c01;
pub(super) const SECONDARY: u128 = 0x0117_7c02;
pub(super) const DIRECT: u128 = 0x0117_7c03;
pub(super) const LINKED: u128 = 0x0117_7c04;
pub(super) const DANGLING: u128 = 0x0117_7c05;
pub(super) const MAGAZINE: u128 = 0x0117_7c06;
const MISSING: u128 = 0x0117_7c07;
const ENTITY: &str = "EntityClassDefinition";
const COMPONENT: &str = "DataForgeComponentParams";
const WEAPON: &str = "SCItemWeaponComponentParams";
const CONTAINER: &str = "SAmmoContainerComponentParams";
const SECONDARY_CONTAINER: &str = "SSecondaryAmmoContainerParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const ATTACHMENT: &str = "SItemDefinition";
const AMMO: &str = "AmmoParams";
const ACTION: &str = "SWeaponActionParams";
const RAPID: &str = "SWeaponActionFireRapidParams";
const LAUNCHER: &str = "SLauncherBase";
const PROJECTILE: &str = "SProjectileLauncher";

pub(super) fn database() -> Vec<u8> {
    action_types(component_types(DataForgeBuilder::new().version(8)))
        .structure(TestStruct::new(AMMO))
        .record(ammo(PRIMARY, "fixture_primary"))
        .record(ammo(SECONDARY, "fixture_secondary"))
        .record(weapon(DIRECT, "direct", "Primary", None))
        .record(weapon(LINKED, "linked", "Secondary", Some(MAGAZINE)))
        .record(weapon(DANGLING, "dangling", "Primary", Some(MISSING)))
        .record(entity(
            MAGAZINE,
            "magazine",
            vec![TestValue::Null, container()],
        ))
        .build()
}

fn component_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(WEAPON)
                .parent(COMPONENT)
                .array("fireActions", pointer(ACTION))
                .field("ammoContainerRecord", reference(ENTITY))
                .array(
                    "secondaryAmmoContainers",
                    TestType::Class(SECONDARY_CONTAINER.into()),
                )
                .field("ShouldIgnorePrimaryAmmoContainer", TestType::Bool),
        )
        .structure(
            TestStruct::new(SECONDARY_CONTAINER).field("ammoContainerRecord", reference(ENTITY)),
        )
        .structure(
            TestStruct::new(CONTAINER)
                .parent(COMPONENT)
                .field("ammoParamsRecord", reference(AMMO))
                .field("secondaryAmmoParamsRecord", reference(AMMO)),
        )
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", TestType::Class(ATTACHMENT.into())),
        )
        .structure(
            TestStruct::new(ATTACHMENT).field("Type", TestType::Enum("FixtureItemType".into())),
        )
}

fn action_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("EProjectileType", ["Primary", "Secondary"])
        .structure(TestStruct::new(ACTION))
        .structure(TestStruct::new(LAUNCHER))
        .structure(
            TestStruct::new(RAPID)
                .parent(ACTION)
                .field("launchParams", pointer(LAUNCHER)),
        )
        .structure(
            TestStruct::new(PROJECTILE)
                .parent(LAUNCHER)
                .field("projectileType", TestType::Enum("EProjectileType".into())),
        )
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn ammo(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        AMMO,
        format!("{AMMO}.{name}"),
        format!("libs/foundry/records/ammoparams/fixture/{name}.xml"),
    )
}

fn entity(guid: u128, name: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        ENTITY,
        format!("{ENTITY}.{name}"),
        format!("libs/foundry/records/entities/scitem/fixture/{name}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn weapon(guid: u128, name: &str, role: &str, linked: Option<u128>) -> TestRecord {
    let attached = TestInstance::new(ATTACHMENT).value("Type", TestValue::text("WeaponGun"));
    let attachable = TestInstance::new(ATTACHABLE).value("AttachDef", TestValue::Struct(attached));
    let launch = TestInstance::new(PROJECTILE).value("projectileType", TestValue::text(role));
    let action = TestInstance::new(RAPID).value("launchParams", TestValue::Struct(launch));
    let weapon = TestInstance::new(WEAPON)
        .value(
            "fireActions",
            TestValue::Array(vec![TestValue::Struct(action)]),
        )
        .value(
            "ammoContainerRecord",
            linked.map_or(TestValue::Null, TestValue::Reference),
        )
        .value("secondaryAmmoContainers", TestValue::Array(Vec::new()))
        .value("ShouldIgnorePrimaryAmmoContainer", TestValue::Bool(false));
    let mut components = vec![TestValue::Struct(attachable), TestValue::Struct(weapon)];
    if linked.is_none() {
        components.push(container());
    }
    entity(guid, name, components)
}

fn container() -> TestValue {
    TestValue::Struct(
        TestInstance::new(CONTAINER)
            .value("ammoParamsRecord", TestValue::Reference(PRIMARY))
            .value("secondaryAmmoParamsRecord", TestValue::Reference(SECONDARY)),
    )
}
