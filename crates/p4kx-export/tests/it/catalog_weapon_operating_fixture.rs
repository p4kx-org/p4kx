// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const ANIMATION: &str = "WeaponProceduralAnimation";
const RECOIL: &str = "ActorProceduralRecoilConfig";
const WEAPON: &str = "SCItemWeaponComponentParams";
const COMPONENT: &str = "DataForgeComponentParams";
const MANNEQUIN: &str = "SAnimationControllerParams";
const CONNECTION: &str = "SWeaponConnectionParams";
const MODIFIER: &str = "SWeaponModifierParams";
const STATS: &str = "SWeaponStats";
const ACTION: &str = "SWeaponActionParams";
const BEAM: &str = "SWeaponActionFireBeamParams";
const ACTION_RECOIL: &str = "WeaponProceduralRecoilConfigDef";
const RECOIL_AIM: &str = "SWeaponProceduralAimRecoilConfigDef";
const ACTION_RECOIL_ID: u128 = 0x9af4;
const AIM: &str = "SWeaponActionAimSimpleParams";
const GEOMETRY: &str = "SWeaponGeometryTagsParams";
const ANIMATION_ID: u128 = 0x9af1;
const MISSING_RECOIL_ID: u128 = 0x9af2;

pub(super) fn database() -> DataForgeBuilder {
    database_with_recoil(DataForgeBuilder::new().version(8), TestStruct::new(RECOIL))
}

fn database_with_recoil(types: DataForgeBuilder, recoil: TestStruct) -> DataForgeBuilder {
    let types = item_types(operating_types(types, recoil));
    types
        .record(TestRecord::new(
            ANIMATION_ID,
            ANIMATION,
            "WeaponProceduralAnimation.Fixture",
            "libs/foundry/records/fixture/weapon_animation.xml",
        ))
        .record(action_recoil())
        .record(item())
}

fn operating_types(types: DataForgeBuilder, recoil: TestStruct) -> DataForgeBuilder {
    types
        .structure(TestStruct::new(ANIMATION))
        .structure(recoil)
        .structure(
            TestStruct::new(BEAM)
                .parent(ACTION)
                .field("recoil", TestType::ReferenceTo(ACTION_RECOIL.to_owned()))
                .field("recoilInterval", TestType::Single),
        )
        .structure(
            TestStruct::new(ACTION_RECOIL).field("weaponProceduralAimRecoil", class(RECOIL_AIM)),
        )
        .structure(TestStruct::new(RECOIL_AIM).field("yawMaxDegrees", TestType::Single))
        .structure(TestStruct::new(ACTION).field("hasReloadModesOnUI", TestType::Bool))
        .structure(
            TestStruct::new(AIM)
                .parent(ACTION)
                .field("aimModifier", pointer(MODIFIER))
                .field("aimGeometryTags", pointer(GEOMETRY)),
        )
        .structure(TestStruct::new(GEOMETRY).field("firstPerson", TestType::Bool))
        .structure(TestStruct::new(MODIFIER).field("weaponStats", class(STATS)))
        .structure(TestStruct::new(STATS).field("damageMultiplier", TestType::Single))
        .structure(
            TestStruct::new(CONNECTION)
                .field("underpowerStats", class(STATS))
                .field("overclockStats", class(STATS))
                .field("overheatLockedStats", pointer(STATS))
                .field("heatRateOnline", TestType::Single),
        )
}

pub(super) const ACTOR_RECOIL_GUID: u128 = 0x9af5;
pub(super) const ACTOR_RECOIL_INDEX: u32 = 3;
pub(super) const ACTOR_MODIFIER_INDEX: u32 = 4;
pub(super) const ACTOR_RECOIL_ALIAS_INDEX: u32 = 5;
const ACTOR_SETUP: &str = "ActorProceduralRecoilSetup";
const ACTOR_MODIFIER: &str = "ActorProceduralRecoilModifiers";
const ACTOR_MODIFIER_GUID: u128 = 0x9af6;
const STANCE: &str = "ActorStateFilterByStanceState";

pub(super) fn database_with_actor_recoil() -> DataForgeBuilder {
    let original = DataForgeBuilder::new()
        .version(8)
        .enumeration(STANCE, ["FixtureStand"])
        .structure(
            TestStruct::new(ACTOR_SETUP)
                .field("filterName", TestType::String)
                .field("filterByStanceState", TestType::Enum(STANCE.to_owned()))
                .field(
                    "actorProceduralRecoilModifiers",
                    TestType::ReferenceTo(ACTOR_MODIFIER.to_owned()),
                )
                .field("fixtureExtra", TestType::Bool),
        )
        .structure(TestStruct::new(ACTOR_MODIFIER).field("kickScale", TestType::Single));
    let recoil = TestStruct::new(RECOIL).array("actorProceduralRecoilSetup", class(ACTOR_SETUP));
    let setup = TestInstance::new(ACTOR_SETUP)
        .value("filterName", TestValue::text("FixtureRecoil"))
        .value("filterByStanceState", TestValue::text("FixtureStand"))
        .value(
            "actorProceduralRecoilModifiers",
            TestValue::Reference(ACTOR_MODIFIER_GUID),
        )
        .value("fixtureExtra", TestValue::Bool(true));
    database_with_recoil(original, recoil)
        .record(actor_recoil_record("Fixture").value(
            "actorProceduralRecoilSetup",
            TestValue::Array(vec![node(setup.clone()), node(setup)]),
        ))
        .record(
            TestRecord::new(
                ACTOR_MODIFIER_GUID,
                ACTOR_MODIFIER,
                "ActorProceduralRecoilModifiers.Fixture",
                "libs/foundry/records/fixture/actor_recoil_modifier.xml",
            )
            .value("kickScale", TestValue::Single(0.5)),
        )
        .record(actor_recoil_record("Alias"))
}

pub(super) fn actor_recoil_record(name: &str) -> TestRecord {
    TestRecord::new(
        ACTOR_RECOIL_GUID,
        RECOIL,
        format!("{RECOIL}.{name}"),
        format!("libs/foundry/records/fixture/actor_recoil_{name}.xml"),
    )
}

fn item_types(types: DataForgeBuilder) -> DataForgeBuilder {
    types
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(TestStruct::new(COMPONENT))
        .structure(
            TestStruct::new(MANNEQUIN)
                .parent(COMPONENT)
                .field("AnimationController", TestType::String)
                .field("AnimationDatabase", TestType::String),
        )
        .structure(TestStruct::new("IWeaponComponentParams").parent(COMPONENT))
        .structure(TestStruct::new("EntityClassDefinition").array("Components", pointer(COMPONENT)))
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
            TestStruct::new(WEAPON)
                .parent("IWeaponComponentParams")
                .field("aimAction", pointer(ACTION))
                .array("fireActions", pointer(ACTION))
                .field(
                    "proceduralAnimationRecord",
                    TestType::ReferenceTo(ANIMATION.to_owned()),
                )
                .field(
                    "actorProceduralRecoilConfig",
                    TestType::ReferenceTo(RECOIL.to_owned()),
                )
                .field("connectionParams", class(CONNECTION))
                .field("weaponDegradationModifier", pointer(MODIFIER)),
        )
}

fn item() -> TestRecord {
    let attach = TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        node(TestInstance::new("SItemDefinition").value("Type", TestValue::text("WeaponGun"))),
    );
    TestRecord::new(
        0x9af3,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureWeaponOperating",
        "libs/foundry/records/entities/scitem/ships/fixture_weapon_operating.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![node(attach), weapon(), mannequin()]),
    )
}

fn mannequin() -> TestValue {
    node(
        TestInstance::new(MANNEQUIN)
            .value(
                "AnimationController",
                TestValue::text("Animations/fixture-controller.xml"),
            )
            .value(
                "AnimationDatabase",
                TestValue::text("Animations/fixture.adb"),
            ),
    )
}

fn weapon() -> TestValue {
    let connection = TestInstance::new(CONNECTION)
        .value("underpowerStats", stats(-0.0))
        .value("overclockStats", stats(2.0))
        .value("overheatLockedStats", TestValue::Null)
        .value("heatRateOnline", TestValue::Single(0.25));
    let aim = TestInstance::new(AIM)
        .value("aimModifier", modifier(0.5))
        .value(
            "aimGeometryTags",
            node(TestInstance::new(GEOMETRY).value("firstPerson", TestValue::Bool(false))),
        )
        .value("hasReloadModesOnUI", TestValue::Bool(false));
    node(
        TestInstance::new(WEAPON)
            .value("aimAction", node(aim))
            .value(
                "fireActions",
                TestValue::Array(vec![beam(0.125), beam(0.25)]),
            )
            .value(
                "proceduralAnimationRecord",
                TestValue::Reference(ANIMATION_ID),
            )
            .value(
                "actorProceduralRecoilConfig",
                TestValue::Reference(MISSING_RECOIL_ID),
            )
            .value("connectionParams", node(connection))
            .value("weaponDegradationModifier", modifier(0.75)),
    )
}

fn beam(interval: f32) -> TestValue {
    node(
        TestInstance::new(BEAM)
            .value("recoil", TestValue::Reference(ACTION_RECOIL_ID))
            .value("recoilInterval", TestValue::Single(interval)),
    )
}

fn action_recoil() -> TestRecord {
    TestRecord::new(
        ACTION_RECOIL_ID,
        ACTION_RECOIL,
        "WeaponProceduralRecoilConfigDef.Fixture",
        "libs/foundry/records/fixture/action_recoil.xml",
    )
    .value(
        "weaponProceduralAimRecoil",
        node(TestInstance::new(RECOIL_AIM).value("yawMaxDegrees", TestValue::Single(1.0))),
    )
}

fn modifier(damage_multiplier: f32) -> TestValue {
    node(TestInstance::new(MODIFIER).value("weaponStats", stats(damage_multiplier)))
}

fn stats(damage_multiplier: f32) -> TestValue {
    node(TestInstance::new(STATS).value("damageMultiplier", TestValue::Single(damage_multiplier)))
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}

fn class(target: &str) -> TestType {
    TestType::Class(target.to_owned())
}

const fn node(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
