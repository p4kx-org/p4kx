// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const OWNER: u32 = 0;
pub(super) const SEQUENCE: &str = "SWeaponActionSequenceParams";
pub(super) const PARALLEL: &str = "SWeaponActionParallelParams";
pub(super) const CONDITIONAL: &str = "SWeaponActionConditionParams";
pub(super) const DYNAMIC: &str = "SWeaponActionDynamicConditionParams";
pub(super) const CHARGED: &str = "SWeaponActionFireChargedParams";
pub(super) const TOGGLE: &str = "SWeaponActionToggleParams";
pub(super) const RAPID: &str = "SWeaponActionFireRapidParams";
pub(super) const BURST: &str = "SWeaponActionFireBurstParams";
pub(super) const SINGLE: &str = "SWeaponActionFireSingleParams";
pub(super) const BEAM: &str = "SWeaponActionFireBeamParams";
pub(super) const AIM: &str = "SWeaponActionAimSimpleParams";
pub(super) const HELD: &str = "SWeaponConditionIsHeldEntity";
pub(super) const MODE: &str = "FixtureSequenceMode";
pub(super) const DELAY_UNIT: &str = "FixtureDelayUnit";

const BASE: &str = "SWeaponActionParams";
const ENTRY: &str = "SWeaponSequenceEntryParams";
const CONDITION_ENTRY: &str = "SConditionalWeaponAction";
const CONDITION: &str = "SWeaponConditionBase";
const WEAPON: &str = "SCItemWeaponComponentParams";

pub(super) fn database() -> DataForgeBuilder {
    let types = action_types(item_types(DataForgeBuilder::new().version(8)));
    types.record(item())
}

fn item_types(types: DataForgeBuilder) -> DataForgeBuilder {
    types
        .enumeration("FixtureItemType", ["WeaponGun"])
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(TestStruct::new("IWeaponComponentParams").parent("DataForgeComponentParams"))
        .structure(
            TestStruct::new("EntityClassDefinition")
                .array("Components", pointer("DataForgeComponentParams")),
        )
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", class("SItemDefinition")),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::Enum("FixtureItemType".to_owned())),
        )
        .structure(
            TestStruct::new(WEAPON)
                .parent("IWeaponComponentParams")
                .field("aimAction", pointer(BASE))
                .array("fireActions", pointer(BASE)),
        )
}

fn action_types(types: DataForgeBuilder) -> DataForgeBuilder {
    let base = types
        .enumeration("ESequenceMode", [MODE])
        .enumeration("EDelayUnit", [DELAY_UNIT])
        .structure(TestStruct::new(BASE).field("hasReloadModesOnUI", TestType::Bool))
        .structure(TestStruct::new(CONDITION))
        .structure(TestStruct::new(HELD).parent(CONDITION))
        .structure(
            action(SEQUENCE)
                .field("mode", TestType::Enum("ESequenceMode".to_owned()))
                .array("sequenceEntries", class(ENTRY)),
        )
        .structure(
            TestStruct::new(ENTRY)
                .field("delay", TestType::Single)
                .field("unit", TestType::Enum("EDelayUnit".to_owned()))
                .field("repetitions", TestType::Int32)
                .field("weaponAction", pointer(BASE)),
        )
        .structure(action(PARALLEL).array("weaponActions", pointer(BASE)))
        .structure(action(TOGGLE).field("weaponAction", pointer(BASE)))
        .structure(condition(CONDITIONAL))
        .structure(condition(DYNAMIC))
        .structure(
            TestStruct::new(CONDITION_ENTRY)
                .field("condition", pointer(CONDITION))
                .field("weaponAction", pointer(BASE)),
        )
        .structure(charge_shape())
        .structure(TestStruct::new("SWeaponStats").field("damageMultiplier", TestType::Single));
    [RAPID, BURST, SINGLE, BEAM, AIM]
        .into_iter()
        .fold(base, |types, name| types.structure(action(name)))
}

fn charge_shape() -> TestStruct {
    action(CHARGED)
        .field("overchargeTime", TestType::Single)
        .field("overchargedTime", TestType::Single)
        .field("chargeAutomatically", TestType::Bool)
        .field("fireOnlyOnFullCharge", TestType::Bool)
        .field("maxChargeModifier", class("SWeaponStats"))
        .field("weaponAction", pointer(BASE))
}

fn condition(name: &str) -> TestStruct {
    action(name)
        .array("conditionalWeaponActions", class(CONDITION_ENTRY))
        .field("defaultWeaponAction", pointer(BASE))
}

fn item() -> TestRecord {
    let attach = node(TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        node(TestInstance::new("SItemDefinition").value("Type", TestValue::text("WeaponGun"))),
    ));
    let actions = node(
        TestInstance::new(WEAPON)
            .value("aimAction", simple(AIM))
            .value(
                "fireActions",
                TestValue::Array(vec![
                    sequence(),
                    parallel(),
                    conditional(),
                    dynamic(),
                    charge(),
                ]),
            ),
    );
    TestRecord::new(
        0x8311,
        "EntityClassDefinition",
        "EntityClassDefinition.FixtureWeaponWrappers",
        "libs/foundry/records/entities/scitem/ships/fixture_weapon_wrappers.xml",
    )
    .value("Components", TestValue::Array(vec![attach, actions]))
}

fn sequence() -> TestValue {
    let entry = |delay, repetitions, selected| {
        node(
            TestInstance::new(ENTRY)
                .value("delay", TestValue::Single(delay))
                .value("unit", TestValue::text(DELAY_UNIT))
                .value("repetitions", TestValue::Int32(repetitions))
                .value("weaponAction", selected),
        )
    };
    node(
        TestInstance::new(SEQUENCE)
            .value("mode", TestValue::text(MODE))
            .value(
                "sequenceEntries",
                TestValue::Array(vec![
                    entry(0.5, 3, simple(RAPID)),
                    entry(-0.0, 0, TestValue::Null),
                ]),
            ),
    )
}

fn parallel() -> TestValue {
    let toggle = node(TestInstance::new(TOGGLE).value("weaponAction", simple(BEAM)));
    node(TestInstance::new(PARALLEL).value(
        "weaponActions",
        TestValue::Array(vec![toggle, TestValue::Null]),
    ))
}

fn conditional() -> TestValue {
    let first = node(
        TestInstance::new(CONDITION_ENTRY)
            .value("condition", simple(HELD))
            .value("weaponAction", simple(BURST)),
    );
    let second = node(
        TestInstance::new(CONDITION_ENTRY)
            .value("condition", TestValue::Null)
            .value("weaponAction", TestValue::Null),
    );
    node(
        TestInstance::new(CONDITIONAL)
            .value(
                "conditionalWeaponActions",
                TestValue::Array(vec![first, second]),
            )
            .value("defaultWeaponAction", simple(BEAM)),
    )
}

fn dynamic() -> TestValue {
    node(
        TestInstance::new(DYNAMIC)
            .value("conditionalWeaponActions", TestValue::Array(Vec::new()))
            .value("defaultWeaponAction", simple(SINGLE)),
    )
}

fn charge() -> TestValue {
    let stats =
        node(TestInstance::new("SWeaponStats").value("damageMultiplier", TestValue::Single(0.0)));
    node(
        TestInstance::new(CHARGED)
            .value("overchargeTime", TestValue::Single(0.25))
            .value("overchargedTime", TestValue::Single(0.5))
            .value("chargeAutomatically", TestValue::Bool(false))
            .value("fireOnlyOnFullCharge", TestValue::Bool(true))
            .value("maxChargeModifier", stats)
            .value("weaponAction", simple(RAPID)),
    )
}

fn simple(name: &str) -> TestValue {
    node(TestInstance::new(name))
}

fn action(name: &str) -> TestStruct {
    TestStruct::new(name).parent(BASE)
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
