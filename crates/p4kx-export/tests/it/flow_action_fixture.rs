// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

use crate::flow_action_values as values;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const OWNER: &str = "f1700000-0000-4000-8000-000000000001";
pub(crate) const BEFORE: &str = "f1700000-0000-4000-8000-000000000011";
pub(crate) const AFTER: &str = "f1700000-0000-4000-8000-000000000022";
pub(crate) const NOTE: &str = "  @raw.trigger %objective% %.2f M\n\\note  ";
pub(crate) const EARLY_VARIABLE: &str = "before-original";
pub(crate) const LATE_VARIABLE: &str = " after-original ";
pub(crate) const NAN: u32 = 0x7fc0_abcd;
pub(crate) const DOUBLE: u64 = 0x3fd0_0000_0000_0001;
const FROM: &[u8] = b"targeT\0";
const TO: &[u8] = b"target\0";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Case {
    Single,
    EqualValues,
    Double,
    Nonfinite,
    UnknownAction,
    NilEmpty,
    Absent,
    Null,
    WrongShape,
    Unreadable,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Names {
    Distinct,
    Duplicate,
}

pub(crate) fn open(case: Case, names: Names, input: Input) -> TestResult<Fixture> {
    let mut bytes = schema(case)
        .record(record(case))
        .record(TestRecord::new(
            0xf170_0000_0000_4000_8000_0000_0000_0002,
            "ContractTemplate",
            "ContractTemplate.Other",
            "libs/foundry/records/contracts/contracttemplates/other_target.xml",
        ))
        .build();
    if matches!(names, Names::Duplicate) {
        let offsets: Vec<_> = bytes
            .windows(FROM.len())
            .enumerate()
            .filter_map(|(index, value)| (value == FROM).then_some(index))
            .collect();
        assert_eq!(offsets.len(), 1, "one real stored second declaration name");
        let start = *offsets.first().ok_or("actual second declaration string")?;
        bytes
            .get_mut(start..start + FROM.len())
            .ok_or("original declaration bytes")?
            .copy_from_slice(TO);
    }
    Fixture::open(&bytes, input, Vec::new())
}

fn schema(case: Case) -> DataForgeBuilder {
    actions(
        conditions(
            DataForgeBuilder::new()
                .version(8)
                .structure(
                    TestStruct::new("ContractTemplate")
                        .field("target", TestType::StrongPointer("FlowTarget".to_owned()))
                        .field("missionFlow", TestType::Class("MissionFlow".to_owned()))
                        .field("targeT", TestType::StrongPointer("FlowTarget".to_owned())),
                )
                .structure(target(case))
                .structure(TestStruct::new("NullableTarget"))
                .structure(TestStruct::new("MissionFlow").array(
                    "triggers",
                    TestType::StrongPointer("FlowTrigger".to_owned()),
                ))
                .structure(
                    TestStruct::new("FlowTrigger")
                        .field("description", TestType::String)
                        .field(
                            "condition",
                            TestType::StrongPointer("MissionFlowCondition".to_owned()),
                        )
                        .field(
                            "action",
                            TestType::StrongPointer("MissionFlowAction".to_owned()),
                        )
                        .field("extraFalse", TestType::Bool),
                ),
        ),
        case,
    )
}

fn conditions(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("MissionFlowCondition"))
        .structure(
            TestStruct::new("MissionFlowCondition_AllTokensState")
                .parent("MissionFlowCondition")
                .field("tokenState", TestType::String),
        )
        .structure(
            TestStruct::new("MissionFlowCondition_TokenState")
                .parent("MissionFlowCondition")
                .field(
                    "objectiveToken",
                    TestType::WeakPointer("FlowTarget".to_owned()),
                )
                .field("tokenState", TestType::String),
        )
        .structure(
            TestStruct::new("MissionFlowCondition_PropertyIsSet")
                .parent("MissionFlowCondition")
                .field("property", TestType::WeakPointer("FlowTarget".to_owned())),
        )
}

fn actions(builder: DataForgeBuilder, case: Case) -> DataForgeBuilder {
    let probability = if matches!(case, Case::Double) {
        TestType::Double
    } else {
        TestType::Single
    };
    builder
        .structure(TestStruct::new("MissionFlowAction"))
        .structure(
            TestStruct::new("MissionFlowAction_StateChange")
                .parent("MissionFlowAction")
                .field("toState", TestType::String),
        )
        .structure(
            TestStruct::new("MissionFlowAction_TokenStateChange")
                .parent("MissionFlowAction")
                .field(
                    "objectiveToken",
                    TestType::WeakPointer("FlowTarget".to_owned()),
                )
                .field("toState", TestType::String),
        )
        .structure(
            TestStruct::new("MissionFlowAction_PickRandomMissionPhase")
                .parent("MissionFlowAction")
                .field("toState", TestType::String)
                .array(
                    "missionPhases",
                    TestType::StrongPointer("MissionRandomPhaseEntry".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("MissionRandomPhaseEntry")
                .field("objective", TestType::WeakPointer("FlowTarget".to_owned()))
                .field("probability", probability),
        )
        .structure(
            TestStruct::new("MissionFlowAction_Mystery")
                .parent("MissionFlowAction")
                .field("toState", TestType::String)
                .field("payload", TestType::String)
                .field("flag", TestType::Bool)
                .field("count", TestType::UInt64),
        )
}

fn target(case: Case) -> TestStruct {
    let target = TestStruct::new("FlowTarget");
    match case {
        Case::Absent => target,
        Case::Null => target
            .field("id", TestType::StrongPointer("NullableTarget".to_owned()))
            .field(
                "missionVariableName",
                TestType::StrongPointer("NullableTarget".to_owned()),
            ),
        Case::WrongShape => target
            .field("id", TestType::Bool)
            .field("missionVariableName", TestType::Bool),
        Case::Unreadable => target
            .field("id", TestType::Bool)
            .field("missionVariableName", TestType::String),
        _ => target
            .field("id", TestType::Guid)
            .field("missionVariableName", TestType::String),
    }
}

fn record(case: Case) -> TestRecord {
    // Named targets are encoded before their weak users; declaration order is independent.
    TestRecord::new(
        0xf170_0000_0000_4000_8000_0000_0000_0001,
        "ContractTemplate",
        "ContractTemplate.SelectedTargets",
        "libs/foundry/records/contracts/contracttemplates/selected_targets.xml",
    )
    .value("target", TestValue::Struct(values::target(case, false)))
    .value("targeT", TestValue::Struct(values::target(case, true)))
    .value("missionFlow", TestValue::Struct(values::flow(case)))
}
