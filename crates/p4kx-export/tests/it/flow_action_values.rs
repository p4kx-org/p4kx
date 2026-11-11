// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestValue};

use crate::flow_action_fixture::{self as fixture, Case};

pub(crate) fn target(case: Case, later: bool) -> TestInstance {
    let name = if later {
        "actual-after"
    } else {
        "actual-before"
    };
    let target = TestInstance::new("FlowTarget").named(name);
    let later = later && !matches!(case, Case::EqualValues);
    match case {
        Case::Absent | Case::Null => target,
        Case::WrongShape => target
            .value("id", TestValue::Bool(true))
            .value("missionVariableName", TestValue::Bool(false)),
        Case::Unreadable => target.value("id", TestValue::Raw(vec![129])).value(
            "missionVariableName",
            TestValue::Raw((u32::MAX - 2).to_le_bytes().to_vec()),
        ),
        Case::NilEmpty => target
            .value("id", TestValue::Guid(0))
            .value("missionVariableName", TestValue::text("")),
        _ => target
            .value(
                "id",
                TestValue::Guid(if later {
                    0xf170_0000_0000_4000_8000_0000_0000_0022
                } else {
                    0xf170_0000_0000_4000_8000_0000_0000_0011
                }),
            )
            .value(
                "missionVariableName",
                TestValue::text(if later {
                    fixture::LATE_VARIABLE
                } else {
                    fixture::EARLY_VARIABLE
                }),
            ),
    }
}

pub(crate) fn flow(case: Case) -> TestInstance {
    let first_action = if matches!(case, Case::UnknownAction) {
        TestInstance::new("MissionFlowAction_Mystery")
            .value("toState", TestValue::text("Active"))
            .value("payload", TestValue::text("  inert @payload %.2f  "))
            .value("flag", TestValue::Bool(false))
            .value("count", TestValue::UInt64(u64::MAX))
    } else {
        TestInstance::new("MissionFlowAction_StateChange")
            .value("toState", TestValue::text("Completed"))
    };
    let all = TestInstance::new("MissionFlowCondition_AllTokensState")
        .value("tokenState", TestValue::text("Active"));
    let token = TestInstance::new("MissionFlowCondition_TokenState")
        .value("objectiveToken", TestValue::pointer("actual-after"))
        .value("tokenState", TestValue::text("Failed"));
    let change = TestInstance::new("MissionFlowAction_TokenStateChange")
        .value("objectiveToken", TestValue::pointer("actual-before"))
        .value("toState", TestValue::text("Active"));
    let property = TestInstance::new("MissionFlowCondition_PropertyIsSet")
        .value("property", TestValue::pointer("actual-after"));
    TestInstance::new("MissionFlow").value(
        "triggers",
        TestValue::Array(vec![
            trigger(fixture::NOTE, all, first_action),
            TestValue::Null,
            trigger("", token, change),
            trigger("random", property, random(case)),
        ]),
    )
}

fn trigger(note: &str, condition: TestInstance, action: TestInstance) -> TestValue {
    TestValue::Struct(
        TestInstance::new("FlowTrigger")
            .value("description", TestValue::text(note))
            .value("condition", TestValue::Struct(condition))
            .value("action", TestValue::Struct(action))
            .value("extraFalse", TestValue::Bool(false)),
    )
}

fn random(case: Case) -> TestInstance {
    TestInstance::new("MissionFlowAction_PickRandomMissionPhase")
        .value("toState", TestValue::text("Completed"))
        .value(
            "missionPhases",
            TestValue::Array(vec![phase(case, false), TestValue::Null, phase(case, true)]),
        )
}

fn phase(case: Case, later: bool) -> TestValue {
    let probability = match case {
        Case::Double => TestValue::Double(if later {
            0.75
        } else {
            f64::from_bits(fixture::DOUBLE)
        }),
        Case::Nonfinite if !later => TestValue::Single(f32::from_bits(fixture::NAN)),
        _ => TestValue::Single(if later { 0.75 } else { -0.0 }),
    };
    TestValue::Struct(
        TestInstance::new("MissionRandomPhaseEntry")
            .value(
                "objective",
                TestValue::pointer(if later {
                    "actual-after"
                } else {
                    "actual-before"
                }),
            )
            .value("probability", probability),
    )
}
