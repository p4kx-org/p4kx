// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Field, Value};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use super::{FlowPlan, FlowRole};
use crate::build::original_input::test_support::{self, TestResult};
use crate::catalog_read::original::OriginalFlowFields;
use crate::template_issues::Unknowns;

#[test]
fn required_condition_precedes_action_even_when_original_fields_are_reversed() -> TestResult {
    test_support::with_input(&bytes(), |forge, input| {
        let record = forge.record(0).ok_or("actual original Template")?;
        let tree = record.tree()?;
        assert_eq!(
            tree.error_count(),
            0,
            "both original unsupported source values are fully readable"
        );
        let scope = input.record_scope(&record, &tree)?;
        let fields = OriginalFlowFields::read(&scope, None)?;
        let before = scope.sources().freeze()?;
        let plan = FlowPlan::read(&fields)?;
        assert_semantic_order(&plan)?;
        assert_eq!(
            scope.sources().freeze()?,
            before,
            "planning alone grants no payload authority"
        );
        let unknowns = Unknowns::default();
        let roots = plan
            .declarations
            .iter()
            .filter_map(|declaration| declaration.declaration);
        let members = crate::template_source_members::TemplateMembers::new(&fields, roots)?;
        let mut references = crate::template_reference_index::TemplateReferences::new(&fields);
        let admission = crate::record_observation::RecordObservation::capture(input, &record)?;
        let capture =
            crate::template_field_capture::TemplateFieldCapture::new(&fields, input, &admission)?;
        let (_, failure) = crate::template_meanings::read(
            &members,
            &plan,
            &mut references,
            Some(&unknowns),
            &capture,
        )?;
        let Some(error) = failure else {
            return Err(
                "the original required meaning must reject the actual unknown condition".into(),
            );
        };
        assert!(
            error.to_string().contains(
                "missionFlow condition holds the unknown struct MissionFlowCondition_Mystery"
            ),
            "the actual shared flat interpreter establishes the same primary cause"
        );
        Ok(())
    })
}

fn assert_semantic_order(plan: &FlowPlan<'_, '_, '_, '_>) -> TestResult {
    assert_eq!(
        plan.semantic_order.len(),
        3,
        "one trigger, condition and action"
    );
    let roles: Vec<_> = plan
        .semantic_order
        .iter()
        .map(|ordinal| plan.roles.get(ordinal).ok_or("actual planned occurrence"))
        .collect::<Result<_, _>>()?;
    let [trigger, condition, action] = roles.as_slice() else {
        return Err("complete actual semantic role sequence".into());
    };
    assert!(
        matches!(trigger.role, FlowRole::Trigger),
        "real trigger first"
    );
    assert!(
        matches!(condition.role, FlowRole::Condition),
        "required condition is read before action"
    );
    assert!(
        matches!(action.role, FlowRole::Action),
        "action interpretation follows the condition"
    );
    assert!(
        action.fact.ordinal() < condition.fact.ordinal(),
        "independent original DFS has the opposite actual field ordering"
    );
    let Value::Struct(value) = trigger.fact.value() else {
        return Err("actual trigger Struct".into());
    };
    assert_eq!(
        value.fields().first().map(Field::name),
        Some("action"),
        "the source really declares action first"
    );
    Ok(())
}

fn bytes() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ContractTemplate")
                .field("missionFlow", TestType::Class("MissionFlow".to_owned())),
        )
        .structure(TestStruct::new("MissionFlow").array(
            "triggers",
            TestType::StrongPointer("FlowTrigger".to_owned()),
        ))
        .structure(
            TestStruct::new("FlowTrigger")
                .field(
                    "action",
                    TestType::StrongPointer("MissionFlowAction".to_owned()),
                )
                .field(
                    "condition",
                    TestType::StrongPointer("MissionFlowCondition".to_owned()),
                ),
        )
        .structure(TestStruct::new("MissionFlowAction"))
        .structure(TestStruct::new("MissionFlowCondition"))
        .structure(TestStruct::new("MissionFlowAction_Mystery").parent("MissionFlowAction"))
        .structure(TestStruct::new("MissionFlowCondition_Mystery").parent("MissionFlowCondition"))
        .record(
            TestRecord::new(
                0xf1b0_0000_0000_4000_8000_0000_0000_0001,
                "ContractTemplate",
                "ContractTemplate.ReadOrder",
                "libs/foundry/records/contracts/contracttemplates/read_order.xml",
            )
            .value(
                "missionFlow",
                TestValue::Struct(TestInstance::new("MissionFlow").value(
                    "triggers",
                    TestValue::Array(vec![TestValue::Struct(
                            TestInstance::new("FlowTrigger")
                                .value(
                                    "action",
                                    TestValue::Struct(TestInstance::new(
                                        "MissionFlowAction_Mystery",
                                    )),
                                )
                                .value(
                                    "condition",
                                    TestValue::Struct(TestInstance::new(
                                        "MissionFlowCondition_Mystery",
                                    )),
                                ),
                        )]),
                )),
            ),
        )
        .build()
}
