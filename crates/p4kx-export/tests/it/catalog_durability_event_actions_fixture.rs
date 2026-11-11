// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::super::{HEALTH, HEALTH_ONLY, database_with_health, item};

const MAELSTROM_BASE: &str = "MaelstromBaseEvent";
const MAELSTROM_DESTROY: &str = "MaelstromDestroyEvent";
const MAELSTROM_STATE: &str = "MaelstromInteractionStateMachineEvent";
const SEQUENCER_BASE: &str = "EntityComponentHealth_SBaseSequencerTask";
const APPLY_DAMAGE_MAP: &str = "EntityComponentHealth_SApplyDamageMapTask";
const UNKNOWN_TASK: &str = "FixtureUnknownHealthTask";
const INTERACTION_STATE: &str = "SInteractionState";
const STATE_INSTANCE: &str = "fixture-state";

pub(super) const TAG: u128 = 0x7310;
pub(super) const MISSING_TAG: u128 = 0x7311;
pub(super) const DAMAGE_PATH: &str = "fixture/damage-map.bin";

pub(super) fn database() -> DataForgeBuilder {
    selected(
        vec![
            node(event(MAELSTROM_BASE).value("groupTag", TestValue::Null)),
            node(event(MAELSTROM_DESTROY).value("groupTag", TestValue::Reference(MISSING_TAG))),
            node(event(MAELSTROM_STATE).value("state", TestValue::pointer(STATE_INSTANCE))),
            TestValue::Null,
        ],
        vec![
            apply_damage_map(),
            node(TestInstance::new(SEQUENCER_BASE)),
            apply_damage_map(),
        ],
    )
}

pub(super) fn unsupported_database() -> DataForgeBuilder {
    selected(
        Vec::new(),
        vec![node(TestInstance::new(UNKNOWN_TASK)), apply_damage_map()],
    )
}

fn selected(events: Vec<TestValue>, tasks: Vec<TestValue>) -> DataForgeBuilder {
    let component = TestInstance::new(HEALTH)
        .value(
            "fixtureLocalState",
            node(
                TestInstance::new(INTERACTION_STATE)
                    .named(STATE_INSTANCE)
                    .value("StateName", TestValue::text("fixture-state-name")),
            ),
        )
        .value("maelstromEvents", TestValue::Array(events))
        .value("sequencerTasks", TestValue::Array(tasks));
    schema()
        .record(item(
            HEALTH_ONLY,
            "durability_authored_events",
            "Misc",
            vec![node(component)],
        ))
        .record(
            TestRecord::new(
                TAG,
                "Tag",
                "Tag.fixture_tag",
                "libs/foundry/records/fixture/fixture_tag.xml",
            )
            .value("tagName", TestValue::text("fixture_tag")),
        )
}

fn schema() -> DataForgeBuilder {
    database_with_health(
        TestStruct::new(HEALTH)
            .parent("DataForgeComponentParams")
            .field("fixtureLocalState", pointer(INTERACTION_STATE))
            .array("maelstromEvents", pointer(MAELSTROM_BASE))
            .array("sequencerTasks", pointer(SEQUENCER_BASE)),
    )
    .structure(TestStruct::new(INTERACTION_STATE).field("StateName", TestType::String))
    .structure(
        TestStruct::new(MAELSTROM_BASE)
            .field("eventTag", TestType::ReferenceTo("Tag".to_owned()))
            .field("groupTag", TestType::ReferenceTo("Tag".to_owned())),
    )
    .structure(TestStruct::new(MAELSTROM_DESTROY).parent(MAELSTROM_BASE))
    .structure(
        TestStruct::new(MAELSTROM_STATE)
            .parent(MAELSTROM_BASE)
            .field("state", TestType::WeakPointer(INTERACTION_STATE.to_owned())),
    )
    .structure(TestStruct::new("SSequencerDefTaskParams"))
    .structure(TestStruct::new(SEQUENCER_BASE).parent("SSequencerDefTaskParams"))
    .structure(
        TestStruct::new(APPLY_DAMAGE_MAP)
            .parent(SEQUENCER_BASE)
            .field("damageMapPath", TestType::String),
    )
    .structure(TestStruct::new(UNKNOWN_TASK).parent(SEQUENCER_BASE))
    .structure(TestStruct::new("Tag").field("tagName", TestType::String))
}

fn event(kind: &str) -> TestInstance {
    TestInstance::new(kind).value("eventTag", TestValue::Reference(TAG))
}

fn apply_damage_map() -> TestValue {
    node(TestInstance::new(APPLY_DAMAGE_MAP).value("damageMapPath", TestValue::text(DAMAGE_PATH)))
}

const fn node(source: TestInstance) -> TestValue {
    TestValue::Struct(source)
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
