// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::super::{HEALTH_ONLY, item};

pub(super) const MACRO: u128 = 0x7201;
pub(super) const GAS: u128 = 0x7202;
pub(super) const TAG: u128 = 0x7203;
pub(super) const MISSING_TAG: u128 = 0x72FF;
pub(super) const WRONG_MACRO: u128 = 0x7204;
pub(super) const WRONG_MACRO_TYPE: &str = "FixtureNonMacro";

pub(super) fn initial_database() -> DataForgeBuilder {
    let base = TestInstance::new("SInitialDamageSpecifierBase");
    let fixed = TestInstance::new("SInitialDamageSpecifierFixed").value(
        "Damage",
        node(TestInstance::new("DamageInfo").value("DamagePhysical", TestValue::Single(45.0))),
    );
    let fraction = TestInstance::new("SInitialDamageSpecifierFraction")
        .value("damageMacro", TestValue::Reference(MACRO))
        .value("HealthFraction", TestValue::Single(0.5));
    initial_schema(base_schema(), reference("DamageMacro"))
        .record(item(
            HEALTH_ONLY,
            "durability_initial_branches",
            "Misc",
            vec![
                initial_health(fixed),
                initial_health(fraction),
                initial_health(base),
            ],
        ))
        .record(record(MACRO, "DamageMacro", "fixture_damage_macro"))
}

pub(super) fn wrong_macro_database(declaration: TestType) -> DataForgeBuilder {
    let fraction = TestInstance::new("SInitialDamageSpecifierFraction")
        .value("damageMacro", TestValue::Reference(WRONG_MACRO))
        .value("HealthFraction", TestValue::Single(0.5));
    initial_schema(base_schema(), declaration)
        .structure(TestStruct::new(WRONG_MACRO_TYPE))
        .record(item(
            HEALTH_ONLY,
            "durability_wrong_initial_macro",
            "Misc",
            vec![initial_health(fraction)],
        ))
        .record(record(WRONG_MACRO, WRONG_MACRO_TYPE, "fixture_non_macro"))
}

pub(super) fn lifecycle_database() -> DataForgeBuilder {
    let health = TestInstance::new("SHealthComponentParams")
        .value("hitManipulationParams", hit())
        .value(
            "HealthLevelStates",
            TestValue::Array(vec![state(true), state(false)]),
        )
        .value(
            "DeathGasComposition",
            TestValue::Array(vec![gas(0.0), gas(2.0)]),
        )
        .value("InteractionLocks", TestValue::Array(vec![lock()]));
    let owner = item(
        HEALTH_ONLY,
        "durability_nested_links",
        "Misc",
        vec![node(health)],
    )
    .value(
        "fixtureInteractionTarget",
        TestValue::pointer("fixture-lock-target"),
    );
    lifecycle_schema(base_schema())
        .instance(
            "fixture-lock-target",
            TestInstance::new("SSharedInteractionParams"),
        )
        .record(owner)
        .record(record(GAS, "BreathableGasParams", "fixture_gas"))
        .record(record(TAG, "Tag", "fixture_tag").value("tagName", TestValue::text("fixture_tag")))
}

fn base_schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FixtureItemType", ["Misc"])
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams"))
                .field(
                    "fixtureInteractionTarget",
                    pointer("SSharedInteractionParams"),
                ),
        )
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(TestStruct::new("SSharedInteractionParams"))
}

fn health_type() -> TestStruct {
    TestStruct::new("SHealthComponentParams").parent("DataForgeComponentParams")
}

fn initial_schema(builder: DataForgeBuilder, damage_macro: TestType) -> DataForgeBuilder {
    builder
        .structure(health_type().field("InitialDamage", pointer("SInitialDamage")))
        .structure(
            TestStruct::new("SInitialDamage")
                .field("RandomSeed", TestType::Int32)
                .field("MinHitCount", TestType::Int32)
                .field("Damage", pointer("SInitialDamageSpecifierBase"))
                .field("BoundingBoxScale", class("Vec3"))
                .field("DamageMapNoiseStrength", class("DamageMapChannels")),
        )
        .structure(TestStruct::new("SInitialDamageSpecifierBase"))
        .structure(
            TestStruct::new("SInitialDamageSpecifierFixed")
                .parent("SInitialDamageSpecifierBase")
                .field("Damage", pointer("DamageBase")),
        )
        .structure(
            TestStruct::new("SInitialDamageSpecifierFraction")
                .parent("SInitialDamageSpecifierBase")
                .field("damageMacro", damage_macro)
                .field("HealthFraction", TestType::Single),
        )
        .structure(TestStruct::new("Vec3").field("z", TestType::Single))
        .structure(TestStruct::new("DamageMapChannels").field("burn", TestType::Single))
        .structure(TestStruct::new("DamageBase"))
        .structure(
            TestStruct::new("DamageInfo")
                .parent("DamageBase")
                .field("DamagePhysical", TestType::Single),
        )
        .structure(TestStruct::new("DamageMacro"))
}

fn lifecycle_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            health_type()
                .field("hitManipulationParams", pointer("HitManipulationParams"))
                .array("HealthLevelStates", class("HealthDamageState"))
                .array("DeathGasComposition", class("SGasMass"))
                .array("InteractionLocks", class("SHealthInteractionLockParams")),
        )
        .structure(
            TestStruct::new("HitManipulationParams")
                .field("ai", class("HitManipulationParamsBase"))
                .field("player", class("HitManipulationParamsBase")),
        )
        .structure(
            TestStruct::new("HitManipulationParamsBase")
                .field("damageMultiplier", TestType::Single),
        )
        .structure(
            TestStruct::new("HealthDamageState")
                .field("geometryTag", TestType::String)
                .field("effectTrigger", reference("Tag"))
                .field("effectTag", reference("Tag")),
        )
        .structure(
            TestStruct::new("Tag")
                .field("tagName", TestType::String)
                .field("legacyGUID", TestType::UInt32)
                .array("children", reference("Tag")),
        )
        .structure(
            TestStruct::new("SGasMass")
                .field("Gas", reference("GasParams"))
                .field("Mass", TestType::Single),
        )
        .structure(TestStruct::new("GasParams"))
        .structure(TestStruct::new("BreathableGasParams").parent("GasParams"))
        .structure(
            TestStruct::new("SHealthInteractionLockParams")
                .field(
                    "Interaction",
                    TestType::WeakPointer("SSharedInteractionParams".to_owned()),
                )
                .field("MinHealthRatio", TestType::Single)
                .field("MaxHealthRatio", TestType::Single),
        )
}

fn initial_health(specifier: TestInstance) -> TestValue {
    node(
        TestInstance::new("SHealthComponentParams").value(
            "InitialDamage",
            node(
                TestInstance::new("SInitialDamage")
                    .value("RandomSeed", TestValue::Int32(-3))
                    .value("MinHitCount", TestValue::Int32(0))
                    .value("Damage", node(specifier))
                    .value(
                        "BoundingBoxScale",
                        node(TestInstance::new("Vec3").value("z", TestValue::Single(1.0))),
                    )
                    .value(
                        "DamageMapNoiseStrength",
                        node(
                            TestInstance::new("DamageMapChannels")
                                .value("burn", TestValue::Single(0.5)),
                        ),
                    ),
            ),
        ),
    )
}

fn hit() -> TestValue {
    let multiplier = |number| {
        node(
            TestInstance::new("HitManipulationParamsBase")
                .value("damageMultiplier", TestValue::Single(number)),
        )
    };
    node(
        TestInstance::new("HitManipulationParams")
            .value("ai", multiplier(0.0))
            .value("player", multiplier(1.25)),
    )
}

fn state(first: bool) -> TestValue {
    let tag = if first {
        TestValue::Reference(MISSING_TAG)
    } else {
        TestValue::Null
    };
    node(
        TestInstance::new("HealthDamageState")
            .value("geometryTag", TestValue::text("fixture-repeat"))
            .value("effectTrigger", TestValue::Reference(TAG))
            .value("effectTag", tag),
    )
}

fn gas(mass: f32) -> TestValue {
    node(
        TestInstance::new("SGasMass")
            .value("Gas", TestValue::Reference(GAS))
            .value("Mass", TestValue::Single(mass)),
    )
}

fn lock() -> TestValue {
    node(
        TestInstance::new("SHealthInteractionLockParams")
            .value("Interaction", TestValue::pointer("fixture-lock-target"))
            .value("MinHealthRatio", TestValue::Single(0.0))
            .value("MaxHealthRatio", TestValue::Single(1.0)),
    )
}

fn record(guid: u128, kind: &str, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.{name}"),
        format!("libs/foundry/records/fixture/{name}.xml"),
    )
}

const fn node(source: TestInstance) -> TestValue {
    TestValue::Struct(source)
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}
