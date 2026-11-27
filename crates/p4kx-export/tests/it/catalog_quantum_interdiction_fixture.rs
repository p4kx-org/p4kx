// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const DEVICE: &str = "SCItemQuantumInterdictionGeneratorParams";
pub(super) const JAMMER: &str = "SCItemQuantumJammerParams";
pub(super) const PULSE: &str = "SCItemQuantumInterdictionPulseParams";
pub(super) const INTERACTION: &str = "SSharedInteractionParams";
const GRAPH: &str = "SQedVisualGraphParams";
const STATE: &str = "SQedVisualGraphStateParams";
const ANIMATION: &str = "SQedVisualGraphAnimationParams";
const EFFECT: &str = "SQedVisualGraphEffectParams";
const TRANSITION: &str = "SQedVisualGraphTransitionParams";
const SELECTOR: &str = "SQedVisualGraphTransitionTypeParams";
const TIME: &str = "SQedVisualGraphTransitionTypeParams_TimeBased";
const VALUE: &str = "SQedVisualGraphTransitionTypeParams_ValueBased";
pub(super) const FUTURE: &str = "FixtureFutureTransition";
const TARGET: &str = "EQedVisualGraphState";
const CONTEXT: &str = "EQedVisualGraphTrackedVariableContext";
const VARIABLE: &str = "EQedVisualGraphTrackedVariable";
const BASE: &str = "DataForgeComponentParams";
const TAG: &str = "Tag";
pub(super) const TAG_GUID: u128 = 0x7101;

pub(super) fn database(wrong_jammer: bool) -> DataForgeBuilder {
    let expected_jammer = if wrong_jammer { PULSE } else { JAMMER };
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("EntityClassDefinition").array("Components", pointer(BASE)))
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(INTERACTION).field("fixtureEnabled", TestType::Bool))
        .structure(TestStruct::new(TAG).field("tagName", TestType::String))
        .structure(
            TestStruct::new(DEVICE)
                .parent(BASE)
                .field("basePowerDrawFraction", TestType::Single)
                .field("jammerSettings", pointer(expected_jammer))
                .field("quantumInterdictionPulseSettings", pointer(PULSE))
                .field("visualGraphParams", class(GRAPH))
                .field("mainDeviceSwitchOn", weak(INTERACTION))
                .field("fixtureExtraField", TestType::Bool),
        )
        .structure(
            TestStruct::new(JAMMER)
                .field("jammerRange", TestType::Single)
                .field("setJammerSwitchOff", weak(INTERACTION)),
        )
        .structure(
            TestStruct::new(PULSE)
                .field("chargeTimeSecs", TestType::Single)
                .field("radiusMeters", TestType::Single)
                .field("activationPhaseDuration_seconds", TestType::Single)
                .field("startChargingIP", weak(INTERACTION)),
        );
    graph_declarations(builder)
        .record(device_record(wrong_jammer))
        .record(
            TestRecord::new(TAG_GUID, TAG, "Tag.fixture", "fixture/tag.xml")
                .value("tagName", TestValue::text("FixtureTag")),
        )
        .record(TestRecord::new(
            0x7102,
            INTERACTION,
            "SSharedInteractionParams.fixture",
            "fixture/interaction.xml",
        ))
}

fn graph_declarations(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(TARGET, ["FixtureTargetStateA", "FixtureTargetStateB"])
        .enumeration(CONTEXT, ["FixtureContext"])
        .enumeration(VARIABLE, ["FixtureVariable"])
        .structure(
            TestStruct::new(GRAPH)
                .field("effectStrengthLink", reference(TAG))
                .array("states", class(STATE)),
        )
        .structure(
            TestStruct::new(STATE)
                .field("animationParams", class(ANIMATION))
                .field("effectParams", class(EFFECT))
                .array("transitions", class(TRANSITION)),
        )
        .structure(
            TestStruct::new(ANIMATION)
                .field("fragmentName", TestType::String)
                .field("fragmentTag", TestType::String)
                .field("loop", TestType::Bool)
                .field("playbackBias", TestType::Single),
        )
        .structure(
            TestStruct::new(EFFECT)
                .field("entityEffectTag", reference(TAG))
                .field("entityEffectTrigger", reference(TAG))
                .field("strength", TestType::Single),
        )
        .structure(
            TestStruct::new(TRANSITION)
                .field("state", enumeration(TARGET))
                .field("animationTransitionType", pointer(SELECTOR))
                .field("effectTransitionType", pointer(SELECTOR)),
        )
        .structure(TestStruct::new(SELECTOR))
        .structure(
            TestStruct::new(TIME)
                .parent(SELECTOR)
                .field("seconds", TestType::Single),
        )
        .structure(
            TestStruct::new(VALUE)
                .parent(SELECTOR)
                .field("context", enumeration(CONTEXT))
                .field("variable", enumeration(VARIABLE)),
        )
        .structure(
            TestStruct::new(FUTURE)
                .parent(SELECTOR)
                .field("fixtureExtension", TestType::Bool),
        )
}

fn device_record(wrong_jammer: bool) -> TestRecord {
    let jammer = if wrong_jammer {
        TestValue::Null
    } else {
        TestValue::Struct(
            TestInstance::new(JAMMER)
                .value("jammerRange", TestValue::Single(28.0))
                .value("setJammerSwitchOff", TestValue::Null),
        )
    };
    let pulse = TestInstance::new(PULSE)
        .value("chargeTimeSecs", TestValue::Single(0.0))
        .value("radiusMeters", TestValue::Single(32.0))
        .value("activationPhaseDuration_seconds", TestValue::Single(2.0))
        .value("startChargingIP", TestValue::Null);
    let device = TestInstance::new(DEVICE)
        .value("basePowerDrawFraction", TestValue::Single(0.5))
        .value("jammerSettings", jammer)
        .value("quantumInterdictionPulseSettings", TestValue::Struct(pulse))
        .value("visualGraphParams", TestValue::Struct(graph()))
        .value(
            "mainDeviceSwitchOn",
            TestValue::pointer("SSharedInteractionParams.fixture"),
        )
        .value("fixtureExtraField", TestValue::Bool(true));
    TestRecord::new(
        0x7100,
        "EntityClassDefinition",
        "EntityClassDefinition.fixture_interdiction",
        "libs/foundry/records/entities/scitem/ships/fixture_interdiction.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(device)]),
    )
}

fn graph() -> TestInstance {
    let animation = TestInstance::new(ANIMATION)
        .value("fragmentName", TestValue::text("fixture"))
        .value("fragmentTag", TestValue::text(""))
        .value("loop", TestValue::Bool(false))
        .value("playbackBias", TestValue::Single(1.0));
    let effect = TestInstance::new(EFFECT)
        .value("entityEffectTag", TestValue::Reference(TAG_GUID))
        .value("entityEffectTrigger", TestValue::Null)
        .value("strength", TestValue::Single(0.7));
    let time = TestInstance::new(TIME).value("seconds", TestValue::Single(0.0));
    let value = TestInstance::new(VALUE)
        .value("context", TestValue::text("FixtureContext"))
        .value("variable", TestValue::text("FixtureVariable"));
    let future = TestInstance::new(FUTURE).value("fixtureExtension", TestValue::Bool(false));
    let first = TestInstance::new(TRANSITION)
        .value("state", TestValue::text("FixtureTargetStateB"))
        .value("animationTransitionType", TestValue::Struct(time))
        .value("effectTransitionType", TestValue::Struct(value));
    let second = TestInstance::new(TRANSITION)
        .value("state", TestValue::text("FixtureTargetStateA"))
        .value("animationTransitionType", TestValue::Struct(future))
        .value("effectTransitionType", TestValue::Null);
    let state = TestInstance::new(STATE)
        .value("animationParams", TestValue::Struct(animation))
        .value("effectParams", TestValue::Struct(effect))
        .value(
            "transitions",
            TestValue::Array(vec![TestValue::Struct(first), TestValue::Struct(second)]),
        );
    TestInstance::new(GRAPH)
        .value("effectStrengthLink", TestValue::Null)
        .value("states", TestValue::Array(vec![TestValue::Struct(state)]))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn weak(name: &str) -> TestType {
    TestType::WeakPointer(name.to_owned())
}

fn reference(name: &str) -> TestType {
    TestType::ReferenceTo(name.to_owned())
}

fn enumeration(name: &str) -> TestType {
    TestType::Enum(name.to_owned())
}
