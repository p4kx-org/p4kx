// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const PRIMARY: u128 = 0x141;
pub(super) const SECONDARY: u128 = 0x142;
pub(super) const WEAPON: u128 = 0x143;
const COMPONENT: &str = "DataForgeComponentParams";
const WEAPON_COMPONENT: &str = "SCItemWeaponComponentParams";
const CONTAINER: &str = "SAmmoContainerComponentParams";
const ACTION: &str = "SWeaponActionParams";
const LAUNCHER: &str = "SLauncherBase";
const PROJECTILE: &str = "SProjectileLauncher";
const RAPID: &str = "SWeaponActionFireRapidParams";
const SECONDARY_MAGAZINE: &str = "SSecondaryAmmoContainerParams";
const CONTAINER_EXTENSION: &str = "FixtureContainerExtension";
const AMMO_EXTENSION: &str = "FixtureAmmoExtension";
const AMMO_EXTENSION_GUID: u128 = 0x144;
const ENTITY: &str = "EntityClassDefinition";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Case {
    Unique,
    SecondWeapon,
    CompetingContainerSubtype,
    UnreadableComponent,
    NoContainer,
    IgnoredPrimary,
    LinkedMagazine,
    SecondaryMagazine,
    UnsupportedRole,
    DanglingAmmo,
    MissingAmmo,
    DerivedAmmo,
    InvalidAction,
    RepeatedAction,
}

pub(super) fn build(case: Case) -> DataForgeBuilder {
    let builder = common_types(DataForgeBuilder::new().version(8))
        .enumeration(
            "EProjectileType",
            ["Primary", "Secondary", "FixtureFutureRole"],
        )
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
        .structure(TestStruct::new("AmmoParams").field("speed", TestType::Single))
        .structure(TestStruct::new(AMMO_EXTENSION).parent("AmmoParams"))
        .record(ammo(PRIMARY, "Primary", 10.0))
        .record(ammo(SECONDARY, "Secondary", 20.0))
        .record(TestRecord::new(
            AMMO_EXTENSION_GUID,
            AMMO_EXTENSION,
            "FixtureAmmoExtension.Unsupported",
            "fixture/ammo/unsupported.xml",
        ));
    builder.record(weapon(case))
}

fn common_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT)))
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new("SAttachableComponentParams")
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
                .field("ammoParamsRecord", reference("AmmoParams"))
                .field("secondaryAmmoParamsRecord", reference("AmmoParams")),
        )
        .structure(TestStruct::new(CONTAINER_EXTENSION).parent(CONTAINER))
}

fn weapon(case: Case) -> TestRecord {
    let attachable = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("SItemDefinition").value("Type", TestValue::text("WeaponGun")),
        ),
    );
    let mut components = vec![
        TestValue::Struct(attachable),
        TestValue::Struct(weapon_component(case)),
    ];
    if case != Case::NoContainer {
        components.push(TestValue::Struct(container(case, CONTAINER)));
    }
    match case {
        Case::SecondWeapon => components.push(TestValue::Struct(weapon_component(Case::Unique))),
        Case::CompetingContainerSubtype => {
            components.push(TestValue::Struct(container(case, CONTAINER_EXTENSION)));
        }
        Case::UnreadableComponent => components.push(invalid_pointer()),
        _ => {}
    }
    TestRecord::new(
        WEAPON,
        ENTITY,
        "EntityClassDefinition.ProjectileFixture",
        "libs/foundry/records/entities/scitem/ships/fixture_projectile_own_route.xml",
    )
    .value("Components", TestValue::Array(components))
}

fn weapon_component(case: Case) -> TestInstance {
    let primary = if case == Case::RepeatedAction {
        fire("Primary").named("original_fixture_action")
    } else {
        fire("Primary")
    };
    let second = match case {
        Case::RepeatedAction => TestValue::pointer("original_fixture_action"),
        Case::InvalidAction => invalid_pointer(),
        Case::UnsupportedRole => TestValue::Struct(fire("FixtureFutureRole")),
        _ => TestValue::Struct(fire("Secondary")),
    };
    let magazine = if case == Case::LinkedMagazine {
        TestValue::Reference(WEAPON)
    } else {
        TestValue::Null
    };
    let secondary = if case == Case::SecondaryMagazine {
        vec![TestValue::Struct(
            TestInstance::new(SECONDARY_MAGAZINE)
                .value("ammoContainerRecord", TestValue::Reference(WEAPON)),
        )]
    } else {
        Vec::new()
    };
    TestInstance::new(WEAPON_COMPONENT)
        .value(
            "fireActions",
            TestValue::Array(vec![TestValue::Struct(primary), second]),
        )
        .value("ammoContainerRecord", magazine)
        .value("secondaryAmmoContainers", TestValue::Array(secondary))
        .value(
            "ShouldIgnorePrimaryAmmoContainer",
            TestValue::Bool(case == Case::IgnoredPrimary),
        )
}

fn fire(role: &str) -> TestInstance {
    TestInstance::new(RAPID).value(
        "launchParams",
        TestValue::Struct(
            TestInstance::new(PROJECTILE).value("projectileType", TestValue::text(role)),
        ),
    )
}

fn container(case: Case, source_type: &str) -> TestInstance {
    let secondary = match case {
        Case::DanglingAmmo => TestValue::Reference(0x149),
        Case::MissingAmmo => TestValue::Null,
        Case::DerivedAmmo => TestValue::Reference(AMMO_EXTENSION_GUID),
        _ => TestValue::Reference(SECONDARY),
    };
    TestInstance::new(source_type)
        .value("ammoParamsRecord", TestValue::Reference(PRIMARY))
        .value("secondaryAmmoParamsRecord", secondary)
}

fn ammo(guid: u128, name: &str, speed: f32) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("fixture/ammo/{name}.xml"),
    )
    .value("speed", TestValue::Single(speed))
}

fn invalid_pointer() -> TestValue {
    let address = [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat();
    TestValue::Raw(address)
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
