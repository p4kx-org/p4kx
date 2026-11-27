// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestStruct, TestType, TestValue, entity_schema,
};

const COMPONENT: &str = "DataForgeComponentParams";
const SHIELD: &str = "SCItemShieldGeneratorParams";
const RANGE: &str = "FixtureShieldRange";
const ATTACHABLE: &str = "EntityComponentAttachableModifierParams";
const EFFECT: &str = "ItemModifiersParams";
const UNSELECTED_EFFECT: &str = "FixtureUnselectedEffect";
const WEAPON_EFFECT: &str = "ItemWeaponModifiersParams";
const WEAPON_MODIFIER: &str = "SWeaponModifierParams";
const WEAPON_STATS: &str = "SWeaponStats";
const SALVAGE: &str = "SSalvageModifier";
const OWNER: u128 = 0x1177_834a;

#[derive(Clone, Copy)]
pub(super) enum Resistance {
    Present,
    Absent,
    Invalid,
}

pub(super) fn database(resistance: Resistance) -> DataForgeBuilder {
    let builder = entity_schema(DataForgeBuilder::new().version(8))
        .structure(shield_schema(resistance))
        .structure(
            TestStruct::new(RANGE)
                .field("Min", TestType::Single)
                .field("Max", TestType::Single),
        );
    salvage_schema(builder).record(
        TestEntity::new(
            OWNER,
            "FixtureShieldSalvage",
            "libs/foundry/records/entities/scitem/gear/shield_salvage.xml",
        )
        .attach_type("Misc")
        .component(shield(9_007_199_254_740_993, resistance))
        .component(attachment())
        .component(shield(128, resistance))
        .record(),
    )
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn shield_schema(resistance: Resistance) -> TestStruct {
    let shield = TestStruct::new(SHIELD)
        .parent(COMPONENT)
        .field("MaxShieldHealth", TestType::UInt64)
        .field("MaxShieldRegen", TestType::Single)
        .field("DownedRegenDelay", TestType::Single)
        .field("DamagedRegenDelay", TestType::Single)
        .field("ElectricalChargeDamageResistance", TestType::Single)
        .field("DecayRatio", TestType::Single);
    match resistance {
        Resistance::Present => shield.array("ShieldResistance", class(RANGE)),
        Resistance::Absent => shield,
        Resistance::Invalid => shield.field("ShieldResistance", TestType::Bool),
    }
}

fn shield(max_health: u64, resistance: Resistance) -> TestValue {
    let shield = TestInstance::new(SHIELD)
        .value("MaxShieldHealth", TestValue::UInt64(max_health))
        .value("MaxShieldRegen", TestValue::Single(12.5))
        .value("DownedRegenDelay", TestValue::Single(4.0))
        .value("DamagedRegenDelay", TestValue::Single(-0.0))
        .value("ElectricalChargeDamageResistance", TestValue::Single(0.25))
        .value("DecayRatio", TestValue::Single(0.125));
    let ranges = [
        range(0.125, 0.75),
        range(0.125, 0.75),
        range(0.5, f32::INFINITY),
    ];
    TestValue::Struct(match resistance {
        Resistance::Present => shield.value("ShieldResistance", TestValue::Array(ranges.into())),
        Resistance::Absent => shield,
        Resistance::Invalid => shield.value("ShieldResistance", TestValue::Bool(false)),
    })
}

fn range(min: f32, max: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(RANGE)
            .value("Min", TestValue::Single(min))
            .value("Max", TestValue::Single(max)),
    )
}

fn salvage_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(ATTACHABLE)
                .parent(COMPONENT)
                .array("modifiers", TestType::StrongPointer(EFFECT.to_owned()))
                .field("charges", TestType::Int32),
        )
        .structure(TestStruct::new(EFFECT))
        .structure(
            TestStruct::new(UNSELECTED_EFFECT)
                .parent(EFFECT)
                .field("unusedField", TestType::Bool),
        )
        .structure(
            TestStruct::new(WEAPON_EFFECT)
                .parent(EFFECT)
                .field("weaponModifier", class(WEAPON_MODIFIER)),
        )
        .structure(TestStruct::new(WEAPON_MODIFIER).field("weaponStats", class(WEAPON_STATS)))
        .structure(
            TestStruct::new(WEAPON_STATS)
                .field("salvageModifier", class(SALVAGE))
                .field("damageMultiplier", TestType::Single),
        )
        .structure(
            TestStruct::new(SALVAGE)
                .field("radiusMultiplier", TestType::Single)
                .field("salvageSpeedMultiplier", TestType::Single)
                .field("extractionEfficiency", TestType::Single),
        )
}

fn attachment() -> TestValue {
    let unused = TestInstance::new(UNSELECTED_EFFECT).value("unusedField", TestValue::Bool(false));
    let salvage = TestInstance::new(SALVAGE)
        .value("radiusMultiplier", TestValue::Single(3.5))
        .value("salvageSpeedMultiplier", TestValue::Single(-0.0))
        .value("extractionEfficiency", TestValue::Single(f32::NAN));
    let stats = TestInstance::new(WEAPON_STATS)
        .value("salvageModifier", TestValue::Struct(salvage))
        .value("damageMultiplier", TestValue::Single(4.0));
    let weapon = TestInstance::new(WEAPON_MODIFIER).value("weaponStats", TestValue::Struct(stats));
    let effect =
        TestInstance::new(WEAPON_EFFECT).value("weaponModifier", TestValue::Struct(weapon));
    TestValue::Struct(
        TestInstance::new(ATTACHABLE)
            .value("charges", TestValue::Int32(3))
            .value(
                "modifiers",
                TestValue::Array(vec![
                    TestValue::Struct(unused),
                    TestValue::Null,
                    TestValue::Struct(effect),
                ]),
            ),
    )
}
