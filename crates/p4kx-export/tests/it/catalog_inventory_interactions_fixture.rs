// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

pub(super) const OWNER: u128 = 0x1057_0400;
pub(super) const PRESET: u128 = 0x1057_0401;
pub(super) const MISSING_PRESET: u128 = 0x1057_0402;
pub(super) const DELAY_BITS: u32 = 0x7fc0_1057;
const BASE: &str = "DataForgeComponentParams";
const COMPONENT: &str = "SEntityInteractableParams";
const INTERACTABLE: &str = "SInteractableParams";
const SHARED: &str = "SSharedInteractionParams";
const FIXTURE_SHARED: &str = "FixtureSharedInteraction";
const POINT: &str = "SInteractionPointParams";
const LINK: &str = "SInteractionParams";
const INTERACTION_TASK: &str = "SSequencerInteractionTask";
const ACTOR_TASK: &str = "SSequencerActorActionHandlerTask";
const ITEM_TASK: &str = "SSequencerItemActionTask";
const MENU_TITLE: &str = "SInteractableMenuTitleParams";
const MACHINE: &str = "SInteractionStateMachineParams";
const STATE_TYPE: &str = "SInteractionStateType";
const STATE: &str = "SInteractionState";
const TRANSITION: &str = "SInteractionStateTransition";
const TRIGGER: &str = "EInteractionTriggerType";
const MODIFIER: &str = "SStateModifier";
const BINDINGS: &str = "InteractionBindingsMethod";
const AUTHORITY: &str = "StateTypeNetworkAuthority";
const CONDITIONS: &str = "InteractionConditionList";
const CONDITION: &str = "InteractionConditionParams";
const CONDITION_PRESET: &str = "InteractionConditionPreset";
const FIXTURE_CONDITION: &str = "FixtureCondition";
const AUTO_CHANGE: &str = "SStateAutoChange";
const CARGO_MODIFIER: &str = "CargoOccupancyStateModifier";
const RAW_MODIFIER: &str = "FixtureStateModifier";

pub(super) fn database() -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    let builder = condition_schema(interaction_schema(builder));
    let builder =
        modifier_schema(state_schema(builder)).instance("fixture-initial", initial_state());
    builder
        .record(
            TestEntity::new(
                OWNER,
                "FixtureInteractionOwner",
                "libs/foundry/records/entities/scitem/fixture/interactions.xml",
            )
            .attach_type("Misc")
            .component(interactable())
            .component(machine())
            .record(),
        )
        .record(TestRecord::new(
            PRESET,
            CONDITION_PRESET,
            "FixtureConditionPreset",
            "libs/foundry/records/fixture/interaction_preset.xml",
        ))
}

fn interaction_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(TRIGGER, ["FixtureTrigger"])
        .structure(
            TestStruct::new(COMPONENT)
                .parent(BASE)
                .field("Interactable", class(INTERACTABLE)),
        )
        .structure(
            TestStruct::new(INTERACTABLE)
                .array("SharedInteractions", strong(SHARED))
                .array("InteractionPoints", class(POINT))
                .array("interactionTasks", strong(INTERACTION_TASK))
                .array("actionHandlerTasks", strong(ACTOR_TASK))
                .array("itemActionTasks", strong(ITEM_TASK))
                .field("menuTitle", strong(MENU_TITLE)),
        )
        .structure(
            TestStruct::new(SHARED)
                .field("Name", TestType::String)
                .field("DisplayName", TestType::Locale)
                .field("available", TestType::Bool)
                .field("conditionList", strong(CONDITIONS)),
        )
        .structure(
            TestStruct::new(FIXTURE_SHARED)
                .parent(SHARED)
                .field("fixturePriority", TestType::UInt16),
        )
        .structure(TestStruct::new(POINT).array("Interactions", strong(LINK)))
        .structure(
            TestStruct::new(LINK)
                .field("Interaction", TestType::WeakPointer(SHARED.to_owned()))
                .field("IsPrimary", TestType::Bool),
        )
        .structure(
            TestStruct::new(INTERACTION_TASK)
                .field("interaction", TestType::String)
                .field("triggerType", TestType::Enum(TRIGGER.to_owned()))
                .field("fixtureActionParameter", TestType::Int32),
        )
        .structure(TestStruct::new(ACTOR_TASK))
        .structure(TestStruct::new(ITEM_TASK))
        .structure(TestStruct::new(MENU_TITLE))
}

fn condition_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(CONDITIONS)
                .array(
                    "presetList",
                    TestType::ReferenceTo(CONDITION_PRESET.to_owned()),
                )
                .array("conditionParams", strong(CONDITION))
                .array("conditionToHideParams", strong(CONDITION)),
        )
        .structure(TestStruct::new(CONDITION))
        .structure(TestStruct::new(CONDITION_PRESET))
        .structure(
            TestStruct::new(FIXTURE_CONDITION)
                .parent(CONDITION)
                .field("fixtureFlag", TestType::Bool),
        )
}

fn modifier_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new(MODIFIER))
        .structure(
            TestStruct::new(CARGO_MODIFIER)
                .parent(MODIFIER)
                .field("cargoVacant", TestType::WeakPointer(STATE.to_owned()))
                .field("cargoStored", TestType::WeakPointer(STATE.to_owned())),
        )
        .structure(
            TestStruct::new(RAW_MODIFIER)
                .parent(MODIFIER)
                .field("fixtureModifierParameter", TestType::UInt16),
        )
        .structure(
            TestStruct::new(AUTO_CHANGE)
                .field("NextState", TestType::WeakPointer(STATE.to_owned()))
                .field("Delay", TestType::Single)
                .field("RunWhenStreamedOut", TestType::Bool),
        )
}

fn state_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .enumeration(BINDINGS, ["FixtureBindings"])
        .enumeration(AUTHORITY, ["FixtureAuthority"])
        .structure(
            TestStruct::new(MACHINE)
                .parent(BASE)
                .array("StateTypes", class(STATE_TYPE)),
        )
        .structure(
            TestStruct::new(STATE_TYPE)
                .field("StateTypeName", TestType::String)
                .field("bindingsMethod", TestType::Enum(BINDINGS.to_owned()))
                .field("DefaultState", TestType::WeakPointer(STATE.to_owned()))
                .field("asopReset", TestType::Bool)
                .field("networkAuthority", TestType::Enum(AUTHORITY.to_owned()))
                .array("States", strong(STATE))
                .array("StateModifiers", strong(MODIFIER)),
        )
        .structure(
            TestStruct::new(STATE)
                .field("StateName", TestType::String)
                .array("Interactions", class(TRANSITION))
                .array("interactionPoints", TestType::WeakPointer(POINT.to_owned()))
                .field("StateAutoChange", strong(AUTO_CHANGE)),
        )
        .structure(
            TestStruct::new(TRANSITION)
                .field("Interaction", TestType::WeakPointer(SHARED.to_owned()))
                .field("NextState", TestType::WeakPointer(STATE.to_owned())),
        )
}

fn interactable() -> TestValue {
    let declaration = TestInstance::new(INTERACTABLE)
        .value(
            "SharedInteractions",
            TestValue::Array(vec![
                interaction("fixture-shared", 7),
                TestValue::Null,
                interaction("fixture-secondary", 8),
            ]),
        )
        .value(
            "InteractionPoints",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new(POINT).named("fixture-point").value(
                    "Interactions",
                    TestValue::Array(vec![TestValue::Struct(
                        TestInstance::new(LINK)
                            .value("Interaction", TestValue::pointer("fixture-shared"))
                            .value("IsPrimary", TestValue::Bool(false)),
                    )]),
                ),
            )]),
        )
        .value(
            "interactionTasks",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new(INTERACTION_TASK)
                    .value("interaction", TestValue::text("FixtureShared"))
                    .value("triggerType", TestValue::text("FixtureTrigger"))
                    .value("fixtureActionParameter", TestValue::Int32(-4)),
            )]),
        )
        .value("actionHandlerTasks", TestValue::Array(Vec::new()))
        .value("itemActionTasks", TestValue::Array(Vec::new()))
        .value("menuTitle", TestValue::Null);
    TestValue::Struct(
        TestInstance::new(COMPONENT).value("Interactable", TestValue::Struct(declaration)),
    )
}

fn interaction(identity: &str, priority: u16) -> TestValue {
    TestValue::Struct(
        TestInstance::new(FIXTURE_SHARED)
            .named(identity)
            .value("Name", TestValue::text("FixtureShared"))
            .value("DisplayName", TestValue::text("@fixture_interaction"))
            .value("available", TestValue::Bool(false))
            .value("conditionList", condition_list())
            .value("fixturePriority", TestValue::UInt16(priority)),
    )
}

fn condition_list() -> TestValue {
    TestValue::Struct(
        TestInstance::new(CONDITIONS)
            .value(
                "presetList",
                TestValue::Array(vec![
                    TestValue::Reference(PRESET),
                    TestValue::Reference(MISSING_PRESET),
                ]),
            )
            .value(
                "conditionParams",
                TestValue::Array(vec![TestValue::Struct(
                    TestInstance::new(FIXTURE_CONDITION)
                        .value("fixtureFlag", TestValue::Bool(false)),
                )]),
            )
            .value("conditionToHideParams", TestValue::Array(Vec::new())),
    )
}

fn auto_change() -> TestValue {
    TestValue::Struct(
        TestInstance::new(AUTO_CHANGE)
            .value("NextState", TestValue::pointer("fixture-initial"))
            .value("Delay", TestValue::Single(f32::from_bits(DELAY_BITS)))
            .value("RunWhenStreamedOut", TestValue::Bool(false)),
    )
}

fn initial_state() -> TestInstance {
    TestInstance::new(STATE)
        .value("StateName", TestValue::text("FixtureState"))
        .value(
            "interactionPoints",
            TestValue::Array(vec![
                TestValue::pointer("fixture-point"),
                TestValue::Null,
                TestValue::pointer("fixture-point"),
            ]),
        )
        .value("StateAutoChange", auto_change())
        .value(
            "Interactions",
            TestValue::Array(vec![TestValue::Struct(
                TestInstance::new(TRANSITION)
                    .value("Interaction", TestValue::pointer("fixture-shared"))
                    .value("NextState", TestValue::pointer("fixture-initial")),
            )]),
        )
}

fn machine() -> TestValue {
    let initial = state_type("FixturePrimary")
        .value("DefaultState", TestValue::pointer("fixture-initial"))
        .value(
            "States",
            TestValue::Array(vec![TestValue::pointer("fixture-initial"), TestValue::Null]),
        )
        .value(
            "StateModifiers",
            TestValue::Array(vec![
                TestValue::Struct(
                    TestInstance::new(CARGO_MODIFIER)
                        .value("cargoVacant", TestValue::pointer("fixture-initial"))
                        .value("cargoStored", TestValue::Null),
                ),
                TestValue::Struct(
                    TestInstance::new(RAW_MODIFIER)
                        .value("fixtureModifierParameter", TestValue::UInt16(41)),
                ),
            ]),
        );
    let empty = state_type("FixtureSecondary")
        .value("DefaultState", TestValue::Null)
        .value("States", TestValue::Array(Vec::new()))
        .value("StateModifiers", TestValue::Array(Vec::new()));
    TestValue::Struct(TestInstance::new(MACHINE).value(
        "StateTypes",
        TestValue::Array(vec![TestValue::Struct(initial), TestValue::Struct(empty)]),
    ))
}

fn state_type(name: &str) -> TestInstance {
    TestInstance::new(STATE_TYPE)
        .value("StateTypeName", TestValue::text(name))
        .value("bindingsMethod", TestValue::text("FixtureBindings"))
        .value("asopReset", TestValue::Bool(false))
        .value("networkAuthority", TestValue::text("FixtureAuthority"))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn strong(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
