// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

const ITEM: &str = "SCItemConsumableParams";
const SUBTYPE: &str = "ConsumableSubtype";
const TYPE: &str = "ConsumableType";
const EFFECT: &str = "ConsumableEffect";
const BASE_UNIT: &str = "SBaseCargoUnit";
const MICRO: &str = "SMicroCargoUnit";
const PACK: &str = "FixtureContents";
const COMPONENT_BASE: &str = "DataForgeComponentParams";
const ENTITY: &str = "EntityClassDefinition";
const HEALTH: &str = "ConsumableEffectHealth";
const STATUS: &str = "ConsumableEffectModifyActorStatus";
const BUFF: &str = "ConsumableEffectAddBuffEffect";
const RESOURCE: &str = "ConsumableEffectResource";
const GAS: &str = "ConsumableEffectGas";
const GAS_MASS: &str = "FixtureGasMass";
const NEW_EFFECT: &str = "FixtureOtherEffect";
const ENUM: &str = "FixtureLabels";

pub(super) fn database() -> DataForgeBuilder {
    let builder = DataForgeBuilder::new().version(8);
    definitions(components(effects(builder)))
        .record(subtype(1, "fixture/subtype/mixed.xml", mixed()))
        .record(subtype(2, "fixture/subtype/empty.xml", Vec::new()))
        .record(subtype(3, "fixture/subtype/unused.xml", Vec::new()))
        .record(
            TestRecord::new(4, TYPE, "FixtureType", "fixture/type/group.xml").value(
                "subtypes",
                TestValue::Array(vec![TestValue::Reference(2), TestValue::Reference(1)]),
            ),
        )
        .record(owner(
            5,
            "fixture/consumable/drink.xml",
            vec![slot(1, 0.25), slot(2, 0.75)],
        ))
        .record(owner(
            6,
            "fixture/consumable/food.xml",
            vec![slot(99, -0.0)],
        ))
        .record(owner(
            7,
            "fixture/consumable/medicine.xml",
            vec![slot(1, 1.0)],
        ))
}

fn definitions(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(SUBTYPE)
                .field("typeName", TestType::String)
                .field("consumableName", TestType::Locale)
                .array("effectsPerMicroSCU", pointer(EFFECT))
                .field("fixtureTypeBackReference", reference(TYPE)),
        )
        .structure(
            TestStruct::new(TYPE)
                .field("typeName", TestType::String)
                .array("subtypes", reference(SUBTYPE)),
        )
}

fn components(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(COMPONENT_BASE))
        .structure(TestStruct::new(ENTITY).array("Components", pointer(COMPONENT_BASE)))
        .structure(TestStruct::new(BASE_UNIT))
        .structure(
            TestStruct::new(MICRO)
                .parent(BASE_UNIT)
                .field("microSCU", TestType::Int32),
        )
        .structure(
            TestStruct::new(PACK)
                .field("consumableSubtype", reference(SUBTYPE))
                .field("ratio", TestType::Single),
        )
        .structure(
            TestStruct::new(ITEM)
                .parent(COMPONENT_BASE)
                .array("defaultContents", class(PACK))
                .field("consumableVolume", pointer(BASE_UNIT))
                .field("containerClosed", TestType::Bool)
                .field("canBeReclosed", TestType::Bool)
                .field("containerTypeTag", TestType::String)
                .field("oneShotConsume", TestType::Bool)
                .field("consumeFragmentId", TestType::String),
        )
}

fn effects(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(ENUM, ["FixtureZero", "FixtureActive"])
        .structure(TestStruct::new(EFFECT).field("effectDescription", TestType::Locale))
        .structure(effect(HEALTH).field("healthChange", TestType::Single))
        .structure(
            effect(STATUS)
                .field("statType", label())
                .field("statPointChange", TestType::Single)
                .field("statCooldownChange", TestType::Single),
        )
        .structure(
            effect(BUFF)
                .field("buffType", label())
                .field("buffDurationOverride", pointer("BuffDurationOverride"))
                .field("buffValueOverride", pointer("BuffValueOverride")),
        )
        .structure(
            TestStruct::new("BuffDurationOverride").field("durationOverride", TestType::Single),
        )
        .structure(
            TestStruct::new("BuffDurationOverrideIndefiniteS42").parent("BuffDurationOverride"),
        )
        .structure(TestStruct::new("BuffValueOverride").field("valueOverride", TestType::Single))
        .structure(
            effect(RESOURCE)
                .field("consumableResourceType", label())
                .field("amount", TestType::Single),
        )
        .structure(effect(GAS).field("gasMass", class(GAS_MASS)))
        .structure(
            TestStruct::new(GAS_MASS)
                .field("Gas", reference("FixtureGas"))
                .field("Mass", TestType::Single),
        )
        .structure(TestStruct::new("FixtureGas"))
        .structure(effect(NEW_EFFECT).field("fixtureExtra", TestType::Int32))
}

fn mixed() -> Vec<TestValue> {
    vec![
        TestValue::Struct(
            TestInstance::new(STATUS)
                .value("statType", TestValue::text("FixtureActive"))
                .value("statPointChange", TestValue::Single(-1.5)),
        ),
        TestValue::Struct(
            TestInstance::new(HEALTH)
                .value("effectDescription", TestValue::text("@own_description"))
                .value("healthChange", TestValue::Single(-0.0)),
        ),
        TestValue::Null,
        TestValue::Struct(
            TestInstance::new(BUFF)
                .value("buffType", TestValue::text("FixtureActive"))
                .value(
                    "buffDurationOverride",
                    TestValue::Struct(TestInstance::new("BuffDurationOverrideIndefiniteS42")),
                )
                .value(
                    "buffValueOverride",
                    TestValue::Struct(
                        TestInstance::new("BuffValueOverride")
                            .value("valueOverride", TestValue::Single(3.0)),
                    ),
                ),
        ),
        TestValue::Struct(
            TestInstance::new(RESOURCE)
                .value("consumableResourceType", TestValue::text("FixtureActive"))
                .value("amount", TestValue::Single(0.25)),
        ),
        TestValue::Struct(
            TestInstance::new(GAS).value(
                "gasMass",
                TestValue::Struct(
                    TestInstance::new(GAS_MASS)
                        .value("Gas", TestValue::Reference(88))
                        .value("Mass", TestValue::Single(0.5)),
                ),
            ),
        ),
        TestValue::Struct(
            TestInstance::new(NEW_EFFECT).value("fixtureExtra", TestValue::Int32(-4)),
        ),
    ]
}

fn subtype(id: u128, path: &str, effects: Vec<TestValue>) -> TestRecord {
    TestRecord::new(id, SUBTYPE, "FixtureSubtype", path)
        .value("typeName", TestValue::text("FixtureLabel"))
        .value("consumableName", TestValue::text("@resource_name"))
        .value("effectsPerMicroSCU", TestValue::Array(effects))
        .value("fixtureTypeBackReference", TestValue::Reference(4))
}

fn owner(id: u128, path: &str, slots: Vec<TestValue>) -> TestRecord {
    let item = TestInstance::new(ITEM)
        .value("defaultContents", TestValue::Array(slots))
        .value("containerTypeTag", TestValue::text(""))
        .value("containerClosed", TestValue::Bool(true))
        .value("canBeReclosed", TestValue::Bool(false))
        .value("oneShotConsume", TestValue::Bool(id == 7))
        .value("consumeFragmentId", TestValue::text("fixture_fragment"))
        .value(
            "consumableVolume",
            TestValue::Struct(TestInstance::new(MICRO).value("microSCU", TestValue::Int32(1234))),
        );
    TestRecord::new(
        id,
        ENTITY,
        format!("EntityClassDefinition.Fixture{id}"),
        path,
    )
    .value(
        "Components",
        TestValue::Array(vec![
            TestValue::Struct(TestInstance::new(COMPONENT_BASE)),
            TestValue::Struct(item),
        ]),
    )
}

fn slot(guid: u128, ratio: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new(PACK)
            .value("consumableSubtype", TestValue::Reference(guid))
            .value("ratio", TestValue::Single(ratio)),
    )
}

fn effect(name: &str) -> TestStruct {
    TestStruct::new(name).parent(EFFECT)
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn label() -> TestType {
    TestType::Enum(ENUM.to_owned())
}
