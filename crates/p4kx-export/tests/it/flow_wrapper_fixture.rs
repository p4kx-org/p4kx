// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(super) const TEMPLATE: u128 = 0x1081_0000_0000_4000_8000_0000_0000_0001;
pub(super) const BROKER: u128 = 0x1081_0000_0000_4000_8000_0000_0000_0002;
pub(super) const TEMPLATE_OBJECTIVE: u128 = TEMPLATE + 16;
pub(super) const BROKER_OBJECTIVE: u128 = BROKER + 16;
pub(super) const MULTIPLE: &str = "MissionFlowAction_MultipleActions";
pub(super) const PICK: &str = "MissionFlowAction_PickRandomAction";
pub(super) const ENTRY: &str = "MissionRandomActionEntry";
pub(super) const MYSTERY: &str = "MissionFlowAction_Mystery";
pub(super) const NAN: u32 = 0x7fc0_1081;

const BASE: &str = "MissionFlowActionBase";
const STATE: &str = "MissionFlowAction_StateChange";
const TOKEN: &str = "MissionFlowAction_TokenStateChange";
const FLOW: &str = "MissionFlow";
const TRIGGER: &str = "MissionFlowTrigger";
const OBJECTIVE: &str = "MissionObjectiveToken";

#[derive(Clone, Copy)]
pub(super) enum Case {
    Mixed,
    Deep(usize),
    AbsentAction,
    AbsentActions,
    UnknownChild,
    UnreadableChild,
    WrongMultipleStorage,
    WrongPickStorage,
    WrongEntry,
    WrongProbability,
}

pub(super) fn open(case: Case, input: Input, both: bool) -> TestResult<Fixture> {
    let mut source = schema(case).record(record("ContractTemplate", TEMPLATE, "template", case));
    if both {
        source = source.record(record("MissionBrokerEntry", BROKER, "broker", case));
    }
    Fixture::open(&source.build(), input, Vec::new())
}

fn schema(case: Case) -> DataForgeBuilder {
    let root = |name| {
        TestStruct::new(name)
            .array(
                "objectiveTokens",
                TestType::StrongPointer(OBJECTIVE.to_owned()),
            )
            .field("missionFlow", TestType::Class(FLOW.to_owned()))
    };
    let builder = DataForgeBuilder::new()
        .version(8)
        .structure(root("ContractTemplate"))
        .structure(root("MissionBrokerEntry"))
        .structure(TestStruct::new(OBJECTIVE).field("id", TestType::Guid))
        .structure(
            TestStruct::new(FLOW).array("triggers", TestType::StrongPointer(TRIGGER.to_owned())),
        )
        .structure(
            TestStruct::new(TRIGGER).field("action", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new(STATE)
                .parent(BASE)
                .field("toState", TestType::String),
        )
        .structure(
            TestStruct::new(TOKEN)
                .parent(BASE)
                .field(
                    "objectiveToken",
                    TestType::WeakPointer(OBJECTIVE.to_owned()),
                )
                .field("toState", TestType::String),
        )
        .structure(
            TestStruct::new(MYSTERY)
                .parent(BASE)
                .field("flag", TestType::Bool),
        );
    wrapper_schema(builder, case)
}

fn wrapper_schema(builder: DataForgeBuilder, case: Case) -> DataForgeBuilder {
    let mut multiple = TestStruct::new(MULTIPLE).parent(BASE);
    if !matches!(case, Case::AbsentActions) {
        let storage = if matches!(case, Case::WrongMultipleStorage) {
            TestType::Class(STATE.to_owned())
        } else {
            TestType::StrongPointer(BASE.to_owned())
        };
        multiple = multiple.array("actions", storage);
    }
    let pick_storage = match case {
        Case::WrongPickStorage => TestType::StrongPointer(ENTRY.to_owned()),
        Case::WrongEntry => TestType::Class("FixtureFutureEntry".to_owned()),
        _ => TestType::Class(ENTRY.to_owned()),
    };
    let probability = if matches!(case, Case::WrongProbability) {
        TestType::Double
    } else {
        TestType::Single
    };
    let mut entry = TestStruct::new(ENTRY).field("probability", probability);
    if !matches!(case, Case::AbsentAction) {
        entry = entry.field("action", TestType::StrongPointer(BASE.to_owned()));
    }
    builder
        .structure(multiple)
        .structure(
            TestStruct::new(PICK)
                .parent(BASE)
                .array("actions", pick_storage),
        )
        .structure(entry)
        .structure(TestStruct::new("FixtureFutureEntry").field("flag", TestType::Bool))
}

fn record(kind: &str, guid: u128, owner: &str, case: Case) -> TestRecord {
    let root = if kind == "ContractTemplate" {
        "libs/foundry/records/contracts/contracttemplates"
    } else {
        "libs/foundry/records/missionbroker"
    };
    let objective = format!("{owner}-objective");
    let target = TestInstance::new(OBJECTIVE)
        .named(&objective)
        .value("id", TestValue::Guid(guid + 16));
    let trigger = TestInstance::new(TRIGGER).value("action", root_action(case, &objective));
    let flow = TestInstance::new(FLOW).value(
        "triggers",
        TestValue::Array(vec![TestValue::Struct(trigger)]),
    );
    TestRecord::new(
        guid,
        kind,
        format!("{kind}.Wrappers"),
        format!("{root}/{owner}_wrappers.xml"),
    )
    .value(
        "objectiveTokens",
        TestValue::Array(vec![TestValue::Struct(target)]),
    )
    .value("missionFlow", TestValue::Struct(flow))
}

fn root_action(case: Case, objective: &str) -> TestValue {
    match case {
        Case::Mixed => mixed(objective),
        Case::Deep(depth) => deep(depth, objective),
        Case::AbsentActions => TestValue::Struct(TestInstance::new(MULTIPLE)),
        Case::AbsentAction => pick(vec![entry(None, TestValue::Single(0.0))]),
        Case::UnknownChild => multiple(vec![pick(vec![entry(
            Some(TestValue::Struct(
                TestInstance::new(MYSTERY).value("flag", TestValue::Bool(false)),
            )),
            TestValue::Single(0.0),
        )])]),
        Case::UnreadableChild => {
            let invalid = [u32::MAX.to_le_bytes(), 0_u32.to_le_bytes()].concat();
            multiple(vec![state(), TestValue::Raw(invalid), state()])
        }
        Case::WrongMultipleStorage => multiple(vec![state()]),
        Case::WrongPickStorage => pick(vec![entry(Some(state()), TestValue::Single(0.0))]),
        Case::WrongEntry => pick(vec![TestValue::Struct(
            TestInstance::new("FixtureFutureEntry").value("flag", TestValue::Bool(true)),
        )]),
        Case::WrongProbability => pick(vec![entry(Some(state()), TestValue::Double(0.25))]),
    }
}

fn mixed(objective: &str) -> TestValue {
    multiple(vec![
        state(),
        TestValue::Null,
        pick(vec![
            entry(Some(token(objective)), TestValue::Single(-0.0)),
            entry(
                Some(TestValue::Null),
                TestValue::Single(f32::from_bits(NAN)),
            ),
            entry(Some(multiple(Vec::new())), TestValue::Single(0.75)),
        ]),
        state(),
    ])
}

fn deep(depth: usize, objective: &str) -> TestValue {
    let mut current = token(objective);
    for index in 0..depth {
        current = if index % 2 == 0 {
            multiple(vec![current])
        } else {
            pick(vec![entry(Some(current), TestValue::Single(0.0))])
        };
    }
    current
}

fn state() -> TestValue {
    TestValue::Struct(TestInstance::new(STATE).value("toState", TestValue::text("Completed")))
}

fn token(objective: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new(TOKEN)
            .value("objectiveToken", TestValue::pointer(objective))
            .value("toState", TestValue::text("Active")),
    )
}

fn multiple(actions: Vec<TestValue>) -> TestValue {
    TestValue::Struct(TestInstance::new(MULTIPLE).value("actions", TestValue::Array(actions)))
}

fn pick(entries: Vec<TestValue>) -> TestValue {
    TestValue::Struct(TestInstance::new(PICK).value("actions", TestValue::Array(entries)))
}

fn entry(action: Option<TestValue>, probability: TestValue) -> TestValue {
    let mut entry = TestInstance::new(ENTRY).value("probability", probability);
    if let Some(action) = action {
        entry = entry.value("action", action);
    }
    TestValue::Struct(entry)
}
