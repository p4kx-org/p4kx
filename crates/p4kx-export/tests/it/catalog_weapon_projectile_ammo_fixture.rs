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
const COUNTERMEASURE_PROJECTILE: &str = "CounterMeasureProjectileParams";
const CHAFF: &str = "CounterMeasureChaffParams";
const FLARE: &str = "CounterMeasureFlareParams";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Case {
    Unique,
    Countermeasure,
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
    MixedModes,
    BeamOnly,
    RepeatedAction,
}

pub(super) fn build(case: Case) -> DataForgeBuilder {
    build_with_secondary_name(case, "Secondary")
}

pub(super) fn cross_owner_ammo(other: bool) -> DataForgeBuilder {
    let database = build_with_secondary_name(Case::Unique, "debug_secondary");
    if !other {
        return database;
    }
    database.record(
        TestRecord::new(
            0x14a,
            ENTITY,
            "EntityClassDefinition.AmmoAdmissionPeer",
            "libs/foundry/records/entities/scitem/fixture/other_weapon.xml",
        )
        .value(
            "Components",
            TestValue::Array(vec![
                TestValue::Struct(weapon_component(Case::IgnoredPrimary)),
                TestValue::Struct(
                    TestInstance::new(CONTAINER)
                        .value("ammoParamsRecord", TestValue::Reference(SECONDARY)),
                ),
            ]),
        ),
    )
}

fn build_with_secondary_name(case: Case, secondary_name: &str) -> DataForgeBuilder {
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
                .field("fireRate", TestType::Int32)
                .field("launchParams", pointer(LAUNCHER)),
        )
        .structure(
            TestStruct::new(PROJECTILE)
                .parent(LAUNCHER)
                .field(
                    "projectileType",
                    TestType::Enum("EProjectileType".to_owned()),
                )
                .field("pelletCount", TestType::Int32)
                .field("damageMultiplier", TestType::Single),
        );
    let builder = [
        "SWeaponActionFireBurstParams",
        "SWeaponActionFireBeamParams",
        "SWeaponActionFireChargedParams",
        "SWeaponActionConditionParams",
    ]
    .into_iter()
    .fold(builder, |builder, name| {
        builder.structure(TestStruct::new(name).parent(ACTION))
    });
    let builder =
        projectile_types(builder).structure(TestStruct::new(AMMO_EXTENSION).parent("AmmoParams"));
    let builder = if case == Case::Countermeasure {
        countermeasure_types(builder)
            .record(countermeasure(PRIMARY, "Primary", CHAFF))
            .record(countermeasure(SECONDARY, "Secondary", FLARE))
    } else {
        builder
            .record(ammo(PRIMARY, "Primary", 10.0))
            .record(ammo(SECONDARY, secondary_name, 20.0))
    };
    builder
        .record(TestRecord::new(
            AMMO_EXTENSION_GUID,
            AMMO_EXTENSION,
            "FixtureAmmoExtension.Unsupported",
            "fixture/ammo/unsupported.xml",
        ))
        .record(weapon(case))
}

fn projectile_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("AmmoParams")
                .field("speed", TestType::Single)
                .field("lifetime", TestType::Single)
                .field("projectileParams", pointer("ProjectileParams")),
        )
        .structure(TestStruct::new("ProjectileParams"))
        .structure(
            TestStruct::new("BulletProjectileParams")
                .parent("ProjectileParams")
                .field("damage", pointer("DamageBase")),
        )
        .structure(TestStruct::new("DamageBase"))
        .structure(
            TestStruct::new("DamageInfo")
                .parent("DamageBase")
                .field("DamagePhysical", TestType::Single)
                .field("DamageEnergy", TestType::Single)
                .field("DamageDistortion", TestType::Single),
        )
}

fn countermeasure_types(builder: DataForgeBuilder) -> DataForgeBuilder {
    let builder = builder
        .structure(
            TestStruct::new(COUNTERMEASURE_PROJECTILE)
                .parent("ProjectileParams")
                .field("typeParams", pointer("CounterMeasureBaseParams")),
        )
        .structure(TestStruct::new("CounterMeasureBaseParams"));
    [CHAFF, FLARE].into_iter().fold(builder, |builder, kind| {
        builder.structure(
            TestStruct::new(kind)
                .parent("CounterMeasureBaseParams")
                .field("StartInfrared", TestType::Single)
                .field("EndCrossSection", TestType::Single),
        )
    })
}

fn countermeasure(guid: u128, name: &str, kind: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("libs/foundry/records/ammoparams/fixture/{name}.xml"),
    )
    .value(
        "projectileParams",
        TestValue::Struct(
            TestInstance::new(COUNTERMEASURE_PROJECTILE).value(
                "typeParams",
                TestValue::Struct(
                    TestInstance::new(kind)
                        .value("StartInfrared", TestValue::Single(5.0))
                        .value("EndCrossSection", TestValue::Single(0.0)),
                ),
            ),
        ),
    )
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
                .field("maxAmmoCount", TestType::Int32)
                .field("maxRestockCount", TestType::UInt32)
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
    let mut actions = if case == Case::BeamOnly {
        vec![TestValue::Struct(TestInstance::new(
            "SWeaponActionFireBeamParams",
        ))]
    } else {
        vec![TestValue::Struct(primary), second]
    };
    if case == Case::Countermeasure {
        actions.push(TestValue::Struct(fire("Primary")));
    }
    if case == Case::MixedModes {
        actions.push(invalid_pointer());
        for kind in [
            "SWeaponActionFireBurstParams",
            "SWeaponActionFireBeamParams",
            "SWeaponActionFireChargedParams",
            "SWeaponActionConditionParams",
        ] {
            actions.push(TestValue::Struct(TestInstance::new(kind)));
        }
    }
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
        .value("fireActions", TestValue::Array(actions))
        .value("ammoContainerRecord", magazine)
        .value("secondaryAmmoContainers", TestValue::Array(secondary))
        .value(
            "ShouldIgnorePrimaryAmmoContainer",
            TestValue::Bool(case == Case::IgnoredPrimary),
        )
}

fn fire(role: &str) -> TestInstance {
    TestInstance::new(RAPID)
        .value(
            "fireRate",
            TestValue::Int32(if role == "Secondary" { 0 } else { 120 }),
        )
        .value(
            "launchParams",
            TestValue::Struct(
                TestInstance::new(PROJECTILE)
                    .value("projectileType", TestValue::text(role))
                    .value("pelletCount", TestValue::Int32(3))
                    .value("damageMultiplier", TestValue::Single(0.5)),
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
        .value("maxAmmoCount", TestValue::Int32(3))
        .value("maxRestockCount", TestValue::UInt32(29))
        .value("ammoParamsRecord", TestValue::Reference(PRIMARY))
        .value("secondaryAmmoParamsRecord", secondary)
}

fn ammo(guid: u128, name: &str, speed: f32) -> TestRecord {
    TestRecord::new(
        guid,
        "AmmoParams",
        format!("AmmoParams.{name}"),
        format!("libs/foundry/records/ammoparams/fixture/{name}.xml"),
    )
    .value("speed", TestValue::Single(speed))
    .value("lifetime", TestValue::Single(2.5))
    .value(
        "projectileParams",
        TestValue::Struct(
            TestInstance::new("BulletProjectileParams").value(
                "damage",
                TestValue::Struct(
                    TestInstance::new("DamageInfo")
                        .value("DamagePhysical", TestValue::Single(speed / 5.0))
                        .value("DamageEnergy", TestValue::Single(6.0))
                        .value("DamageDistortion", TestValue::Single(0.0)),
                ),
            ),
        ),
    )
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
