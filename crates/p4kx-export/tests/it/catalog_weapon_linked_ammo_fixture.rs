// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const PRIMARY: u128 = 0x801;
pub(super) const SECONDARY: u128 = 0x802;
pub(super) const WEAPON: u128 = 0x803;
pub(super) const MAGAZINE: u128 = 0x804;
pub(super) const FILTER: &str = "*/fixture_linked_projectile";
const COMPONENT: &str = "DataForgeComponentParams";
const WEAPON_COMPONENT: &str = "SCItemWeaponComponentParams";
const CONTAINER: &str = "SAmmoContainerComponentParams";
const CONTAINER_EXTENSION: &str = "FixtureContainerExtension";
const ACTION: &str = "SWeaponActionParams";
const RAPID: &str = "SWeaponActionFireRapidParams";
const LAUNCHER: &str = "SLauncherBase";
const PROJECTILE: &str = "SProjectileLauncher";
const SECONDARY_MAGAZINE: &str = "SSecondaryAmmoContainerParams";
const ATTACHABLE: &str = "SAttachableComponentParams";
const ENTITY: &str = "EntityClassDefinition";
const AMMO: &str = "AmmoParams";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Case {
    Unique,
    NoContainer,
    MultipleContainers,
    TrailingUnreadable,
    DuplicateGuid,
    Dangling,
    WrongType,
    OwnSubtype,
    OwnWithLink,
    Conditional,
    SelfLink,
    MissingSecondary,
}

pub(super) fn build(case: Case) -> DataForgeBuilder {
    with_ammo_names(case, "Primary", "Secondary")
}

pub(super) fn with_ammo_names(
    case: Case,
    primary_name: &str,
    secondary_name: &str,
) -> DataForgeBuilder {
    let builder = common_types(DataForgeBuilder::new().version(8))
        .enumeration("EProjectileType", ["Primary", "Secondary"])
        .structure(TestStruct::new(ACTION))
        .structure(TestStruct::new(LAUNCHER))
        .structure(
            TestStruct::new(RAPID)
                .parent(ACTION)
                .field("launchParams", pointer(LAUNCHER)),
        )
        .structure(TestStruct::new(PROJECTILE).parent(LAUNCHER).field(
            "projectileType",
            TestType::Enum("EProjectileType".to_owned()),
        ))
        .structure(TestStruct::new(AMMO).field("speed", TestType::Single))
        .record(ammo(PRIMARY, primary_name, 10.0))
        .record(ammo(SECONDARY, secondary_name, 20.0))
        .record(weapon(case));
    match case {
        Case::Dangling | Case::SelfLink => builder,
        Case::WrongType => builder.record(ammo(MAGAZINE, "Wrong", 30.0)),
        Case::DuplicateGuid => builder
            .record(magazine(case, "first"))
            .record(magazine(case, "second")),
        _ => builder.record(magazine(case, "unique")),
    }
}

fn common_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(
            TestStruct::new(WEAPON_COMPONENT)
                .parent(COMPONENT)
                .array("fireActions", pointer(ACTION))
                .field("ammoContainerRecord", reference(ENTITY))
                .array("secondaryAmmoContainers", class(SECONDARY_MAGAZINE))
                .field("ShouldIgnorePrimaryAmmoContainer", TestType::Bool),
        )
        .structure(
            TestStruct::new(SECONDARY_MAGAZINE).field("ammoContainerRecord", reference(ENTITY)),
        )
        .structure(
            TestStruct::new(CONTAINER)
                .parent(COMPONENT)
                .field("ammoParamsRecord", reference(AMMO))
                .field("secondaryAmmoParamsRecord", reference(AMMO)),
        )
        .structure(TestStruct::new(CONTAINER_EXTENSION).parent(CONTAINER))
}

fn weapon(case: Case) -> TestRecord {
    let attachable = TestValue::Struct(TestInstance::new(ATTACHABLE).value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("SItemDefinition").value("Type", TestValue::text("WeaponGun")),
        ),
    ));
    let linked = if case == Case::SelfLink {
        WEAPON
    } else {
        MAGAZINE
    };
    let secondary = if case == Case::Conditional {
        vec![TestValue::Struct(
            TestInstance::new(SECONDARY_MAGAZINE)
                .value("ammoContainerRecord", TestValue::Reference(MAGAZINE)),
        )]
    } else {
        Vec::new()
    };
    let weapon = TestInstance::new(WEAPON_COMPONENT)
        .value(
            "fireActions",
            TestValue::Array(vec![fire("Primary"), fire("Secondary")]),
        )
        .value("ammoContainerRecord", TestValue::Reference(linked))
        .value("secondaryAmmoContainers", TestValue::Array(secondary))
        .value("ShouldIgnorePrimaryAmmoContainer", TestValue::Bool(false));
    let mut components = vec![attachable, TestValue::Struct(weapon)];
    match case {
        Case::OwnSubtype => components.push(container(case, CONTAINER_EXTENSION)),
        Case::OwnWithLink => components.push(container(case, CONTAINER)),
        _ => {}
    }
    TestRecord::new(
        WEAPON,
        ENTITY,
        "EntityClassDefinition.LinkedProjectileFixture",
        "libs/foundry/records/entities/scitem/ships/fixture_linked_projectile.xml",
    )
    .value("Components", TestValue::Array(components))
}

fn magazine(case: Case, id: &str) -> TestRecord {
    let mut components = vec![TestValue::Null];
    if case != Case::NoContainer {
        components.push(container(case, CONTAINER));
    }
    match case {
        Case::MultipleContainers => components.push(container(case, CONTAINER_EXTENSION)),
        Case::TrailingUnreadable => components.push(TestValue::Raw(
            [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat(),
        )),
        _ => {}
    }
    TestRecord::new(
        MAGAZINE,
        ENTITY,
        format!("EntityClassDefinition.FixtureMagazine_{id}"),
        format!("fixture/magazine/{id}.xml"),
    )
    .value("Components", TestValue::Array(components))
}

fn fire(role: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(RAPID).value(
        "launchParams",
        TestValue::Struct(
            TestInstance::new(PROJECTILE).value("projectileType", TestValue::text(role)),
        ),
    ))
}

fn container(case: Case, source_type: &str) -> TestValue {
    let secondary = if case == Case::MissingSecondary {
        TestValue::Null
    } else {
        TestValue::Reference(SECONDARY)
    };
    TestValue::Struct(
        TestInstance::new(source_type)
            .value("ammoParamsRecord", TestValue::Reference(PRIMARY))
            .value("secondaryAmmoParamsRecord", secondary),
    )
}

fn ammo(guid: u128, name: &str, speed: f32) -> TestRecord {
    TestRecord::new(
        guid,
        AMMO,
        format!("AmmoParams.{name}"),
        format!("fixture/ammo/{name}.xml"),
    )
    .value("speed", TestValue::Single(speed))
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}
