// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const GUID: &str = "f1800000-0000-4000-8000-000000000001";
pub(crate) const ID: &str = "f1800000-0000-4000-8000-000000000033";
pub(crate) const NAN: u64 = 0x7ff8_0000_0000_abcd;
pub(crate) const VARIABLE: &str = " exact-selected-variable ";

#[derive(Debug, Clone, Copy)]
pub(crate) enum Case {
    InsideFlow,
    Overlap,
}

pub(crate) fn open(case: Case, input: Input) -> TestResult<Fixture> {
    open_owner(
        case,
        input,
        "ContractTemplate",
        "ContractTemplate.ReferencedRoots",
        "libs/foundry/records/contracts/contracttemplates/referenced_roots.xml",
    )
}

pub(crate) fn open_broker(case: Case, input: Input) -> TestResult<Fixture> {
    open_owner(
        case,
        input,
        "MissionBrokerEntry",
        "MissionBrokerEntry.ReferencedRoots",
        "libs/foundry/records/missionbroker/observed/referenced_roots.xml",
    )
}

fn open_owner(
    case: Case,
    input: Input,
    owner: &str,
    name: &str,
    path: &str,
) -> TestResult<Fixture> {
    let bytes = schema(case, owner)
        .record(record(case, owner, name, path))
        .build();
    Fixture::open(&bytes, input, Vec::new())
}

fn schema(case: Case, owner: &str) -> DataForgeBuilder {
    let mut root = TestStruct::new(owner);
    if matches!(case, Case::Overlap) {
        root = root.field("target", TestType::StrongPointer("FlowTarget".to_owned()));
    }
    root = root.field("missionFlow", TestType::Class("MissionFlow".to_owned()));
    let mut flow = TestStruct::new("MissionFlow");
    if matches!(case, Case::InsideFlow) {
        flow = flow.field("target", TestType::StrongPointer("FlowTarget".to_owned()));
    }
    flow = flow.array("triggers", TestType::Class("FlowTrigger".to_owned()));
    let id = if matches!(case, Case::Overlap) {
        TestType::StrongPointer("NestedTarget".to_owned())
    } else {
        TestType::Guid
    };
    let property = if matches!(case, Case::Overlap) {
        "NestedTarget"
    } else {
        "FlowTarget"
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(root)
        .structure(flow)
        .structure(
            TestStruct::new("FlowTarget")
                .field("id", id)
                .field("missionVariableName", TestType::String),
        )
        .structure(
            TestStruct::new("NestedTarget")
                .field("missionVariableName", TestType::String)
                .field("nonfinite", TestType::Double)
                .field("flag", TestType::Bool),
        )
        .structure(TestStruct::new("FlowTrigger").field(
            "condition",
            TestType::StrongPointer("MissionFlowCondition".to_owned()),
        ))
        .structure(TestStruct::new("MissionFlowCondition"))
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
                .field("property", TestType::WeakPointer(property.to_owned())),
        )
}

fn record(case: Case, owner: &str, name: &str, path: &str) -> TestRecord {
    let record = TestRecord::new(0xf180_0000_0000_4000_8000_0000_0000_0001, owner, name, path);
    let record = if matches!(case, Case::Overlap) {
        record.value("target", TestValue::Struct(target(case)))
    } else {
        record
    };
    record.value("missionFlow", TestValue::Struct(flow(case)))
}

fn target(case: Case) -> TestInstance {
    let id = if matches!(case, Case::Overlap) {
        TestValue::Struct(
            TestInstance::new("NestedTarget")
                .named("selected-nested")
                .value("missionVariableName", TestValue::text(VARIABLE))
                .value("nonfinite", TestValue::Double(f64::from_bits(NAN)))
                .value("flag", TestValue::Bool(false)),
        )
    } else {
        TestValue::Guid(0xf180_0000_0000_4000_8000_0000_0000_0033)
    };
    TestInstance::new("FlowTarget")
        .named("selected-owner")
        .value("id", id)
        .value(
            "missionVariableName",
            TestValue::text(if matches!(case, Case::InsideFlow) {
                VARIABLE
            } else {
                "unselected-owner-variable"
            }),
        )
}

fn flow(case: Case) -> TestInstance {
    let flow = if matches!(case, Case::InsideFlow) {
        TestInstance::new("MissionFlow").value("target", TestValue::Struct(target(case)))
    } else {
        TestInstance::new("MissionFlow")
    };
    let objective = TestInstance::new("MissionFlowCondition_TokenState")
        .value("objectiveToken", TestValue::pointer("selected-owner"))
        .value("tokenState", TestValue::text("Active"));
    let property = TestInstance::new("MissionFlowCondition_PropertyIsSet").value(
        "property",
        TestValue::pointer(if matches!(case, Case::Overlap) {
            "selected-nested"
        } else {
            "selected-owner"
        }),
    );
    flow.value(
        "triggers",
        TestValue::Array(vec![
            TestValue::Struct(
                TestInstance::new("FlowTrigger").value("condition", TestValue::Struct(objective)),
            ),
            TestValue::Struct(
                TestInstance::new("FlowTrigger").value("condition", TestValue::Struct(property)),
            ),
        ]),
    )
}
