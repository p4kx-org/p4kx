// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataType, Record, StructValue, Value};

use crate::flow_action_fixture::{self as fixture, Case, Names};
use crate::flow_action_source as source;
use crate::flow_action_target_source;
use crate::health_origin_fixture::TestResult;

pub(crate) fn check(
    root: &StructValue<'_>,
    case: Case,
    names: Names,
    record: &Record<'_>,
) -> TestResult {
    assert_eq!(root.type_name(), "MissionFlow", "actual Flow type");
    let triggers = source::array(root, "triggers")?;
    assert_eq!(
        triggers.len(),
        4,
        "all actual trigger slots remain materialized"
    );
    assert_eq!(
        triggers.get(1),
        Some(&Value::Null),
        "actual skipped null trigger is still source data"
    );
    let first = source::structure(triggers.first().ok_or("actual first trigger")?)?;
    assert_eq!(
        first.field("description"),
        Some(&Value::String(fixture::NOTE)),
        "exact original trigger note stays inert"
    );
    let middle = source::structure(triggers.get(2).ok_or("actual token trigger")?)?;
    assert_eq!(
        middle.field("description"),
        Some(&Value::String("")),
        "empty trigger note is a real source string"
    );
    let last = source::structure(triggers.get(3).ok_or("actual random trigger")?)?;
    for trigger in [first, middle, last] {
        assert_eq!(
            trigger.field("extraFalse"),
            Some(&Value::Bool(false)),
            "additional original trigger fact"
        );
    }
    routes(middle, last, names, record)?;
    random(
        source::structure(last.field("action").ok_or("actual pick action")?)?,
        case,
        record,
    )?;
    first_action(
        source::structure(first.field("action").ok_or("actual first action")?)?,
        case,
    );
    Ok(())
}

fn first_action(action: &StructValue<'_>, case: Case) {
    if matches!(case, Case::UnknownAction) {
        assert_eq!(
            action.type_name(),
            "MissionFlowAction_Mystery",
            "genuine readable required unknown action"
        );
        assert_eq!(
            action.field("payload"),
            Some(&Value::String("  inert @payload %.2f  ")),
            "unknown action has actual retained payload"
        );
        assert_eq!(
            action.field("flag"),
            Some(&Value::Bool(false)),
            "unknown action false fact"
        );
        assert_eq!(
            action.field("count"),
            Some(&Value::UInt64(u64::MAX)),
            "unknown action exact wide integer"
        );
    } else {
        assert_eq!(
            action.type_name(),
            "MissionFlowAction_StateChange",
            "actual known state-change action"
        );
    }
    assert_eq!(
        action.field("toState"),
        Some(&Value::String(if matches!(case, Case::UnknownAction) {
            "Active"
        } else {
            "Completed"
        })),
        "actual state label before interpretation"
    );
}

fn random(action: &StructValue<'_>, case: Case, record: &Record<'_>) -> TestResult {
    assert_eq!(
        action.type_name(),
        "MissionFlowAction_PickRandomMissionPhase",
        "actual pick action"
    );
    let phases = source::array(action, "missionPhases")?;
    assert_eq!(phases.len(), 3, "original phase array slots");
    assert_eq!(phases.get(1), Some(&Value::Null), "real null phase slot");
    for (slot, position) in [(0, 0), (2, 2)] {
        let phase = source::structure(phases.get(slot).ok_or("actual original phase")?)?;
        assert_eq!(
            phase.type_name(),
            "MissionRandomPhaseEntry",
            "actual random phase type"
        );
        flow_action_target_source::reference(phase, "objective", record, position)?;
        let field = phase
            .fields()
            .get(1)
            .ok_or("actual probability declaration")?;
        match field.value() {
            Value::Double(value) if matches!(case, Case::Double) => {
                assert_eq!(
                    field.storage().data_type(),
                    DataType::Double,
                    "genuine Double probability storage"
                );
                assert_eq!(
                    value.to_bits(),
                    if slot == 0 {
                        fixture::DOUBLE
                    } else {
                        0.75_f64.to_bits()
                    },
                    "all original Double bits, including beyond-f32 precision"
                );
            }
            Value::Single(value) => {
                assert_eq!(
                    field.storage().data_type(),
                    DataType::Single,
                    "genuine Single probability storage"
                );
                let expected = if slot == 2 {
                    0.75_f32.to_bits()
                } else if matches!(case, Case::Nonfinite) {
                    fixture::NAN
                } else {
                    (-0.0_f32).to_bits()
                };
                assert_eq!(
                    value.to_bits(),
                    expected,
                    "exact Single/NaN/negative-zero bits"
                );
            }
            _ => return Err("actual original probability width/value".into()),
        }
    }
    Ok(())
}

fn routes(
    middle: &StructValue<'_>,
    last: &StructValue<'_>,
    names: Names,
    record: &Record<'_>,
) -> TestResult {
    let condition = source::structure(middle.field("condition").ok_or("actual token condition")?)?;
    let action = source::structure(middle.field("action").ok_or("actual token action")?)?;
    assert_eq!(
        condition.type_name(),
        "MissionFlowCondition_TokenState",
        "actual token condition kind"
    );
    assert_eq!(
        condition.field("tokenState"),
        Some(&Value::String("Failed")),
        "actual condition state label"
    );
    assert_eq!(
        action.type_name(),
        "MissionFlowAction_TokenStateChange",
        "actual token action kind"
    );
    assert_eq!(
        action.field("toState"),
        Some(&Value::String("Active")),
        "actual action state label"
    );
    let later = flow_action_target_source::reference(condition, "objectiveToken", record, 2)?;
    let before = flow_action_target_source::reference(action, "objectiveToken", record, 0)?;
    let before_location = before.location().ok_or("actual earlier location")?;
    let later_location = later.location().ok_or("actual later location")?;
    assert_eq!(
        before_location.as_str(),
        "target",
        "actual earlier source path"
    );
    assert_eq!(
        later_location.as_str(),
        if matches!(names, Names::Duplicate) {
            "target"
        } else {
            "targeT"
        },
        "actual later source path"
    );
    if matches!(names, Names::Duplicate) {
        assert_eq!(
            before_location, later_location,
            "equal descriptive paths are not original target identity"
        );
        assert_ne!(
            before.original_target().instance_positions(),
            later.original_target().instance_positions(),
            "different original targets need typed positions"
        );
    }
    let property = source::structure(last.field("condition").ok_or("actual property condition")?)?;
    flow_action_target_source::reference(property, "property", record, 2)?;
    Ok(())
}
