// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const GUID: &str = "f1500000-0000-4000-8000-000000000001";
const BASE: &str = "MissionFlowCondition";

/// The condition follows the template, flow and trigger source levels.
pub(crate) const CONDITION_SOURCE_DEPTH: usize = 3;

#[derive(Clone, Copy)]
pub(crate) enum Branch {
    Not,
    And,
    Or,
}

impl Branch {
    pub(crate) const fn source_type(self) -> &'static str {
        match self {
            Self::Not => "MissionFlowCondition_NOT",
            Self::And => "MissionFlowCondition_AND",
            Self::Or => "MissionFlowCondition_OR",
        }
    }

    pub(crate) const fn wire_type(self) -> &'static str {
        match self {
            Self::Not => "not",
            Self::And => "and",
            Self::Or => "or",
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) enum Tail {
    Known,
    Unknown,
    Missing,
    Empty,
}

pub(crate) fn open(branch: Branch, count: usize, tail: Tail) -> TestResult<Fixture> {
    archive(chain(branch, count, tail))
}

pub(crate) fn chain(branch: Branch, count: usize, tail: Tail) -> TestInstance {
    let mut condition = match tail {
        Tail::Known => TestInstance::new("MissionFlowCondition_OnMissionStart"),
        Tail::Unknown => TestInstance::new("MissionFlowCondition_Mystery"),
        Tail::Missing => TestInstance::new("MissionFlowCondition_NOT"),
        Tail::Empty => TestInstance::new("MissionFlowCondition_AND"),
    };
    for _ in 0..count {
        let child = TestValue::Struct(condition);
        condition = TestInstance::new(branch.source_type());
        condition = match branch {
            Branch::Not => condition.value("condition", child),
            Branch::And | Branch::Or => {
                condition.value("conditions", TestValue::Array(vec![child]))
            }
        };
    }
    condition
}

pub(crate) fn flow(condition: TestInstance) -> TestValue {
    let trigger = TestInstance::new("FlowTrigger").value("condition", TestValue::Struct(condition));
    let flow = TestInstance::new("MissionFlow").value(
        "triggers",
        TestValue::Array(vec![TestValue::Struct(trigger)]),
    );
    TestValue::Struct(flow)
}

fn archive(condition: TestInstance) -> TestResult<Fixture> {
    let bytes = schema()
        .record(
            TestRecord::new(
                0xf150_0000_0000_4000_8000_0000_0000_0001,
                "ContractTemplate",
                "ContractTemplate.CompleteFlow",
                "libs/foundry/records/contracts/contracttemplates/complete_flow.xml",
            )
            .value("missionFlow", flow(condition)),
        )
        .build();
    Fixture::open(&bytes, Input::Plain, Vec::new())
}

pub(crate) fn schema() -> DataForgeBuilder {
    let pointer = || TestType::StrongPointer(BASE.to_owned());
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ContractTemplate")
                .field("missionFlow", TestType::Class("MissionFlow".to_owned())),
        )
        .structure(
            TestStruct::new("MissionFlow")
                .array("triggers", TestType::Class("FlowTrigger".to_owned())),
        )
        .structure(TestStruct::new("FlowTrigger").field("condition", pointer()))
        .structure(TestStruct::new(BASE))
        .structure(
            TestStruct::new("MissionFlowCondition_NOT")
                .parent(BASE)
                .field("condition", pointer()),
        )
        .structure(
            TestStruct::new("MissionFlowCondition_AND")
                .parent(BASE)
                .array("conditions", pointer()),
        )
        .structure(
            TestStruct::new("MissionFlowCondition_OR")
                .parent(BASE)
                .array("conditions", pointer()),
        )
        .structure(TestStruct::new("MissionFlowCondition_OnMissionStart").parent(BASE))
        .structure(TestStruct::new("MissionFlowCondition_Mystery").parent(BASE))
}

pub(crate) fn mixed() -> TestResult<Fixture> {
    let alternatives = TestInstance::new("MissionFlowCondition_OR").value(
        "conditions",
        TestValue::Array(vec![
            TestValue::Struct(TestInstance::new("MissionFlowCondition_NOT")),
            TestValue::Struct(TestInstance::new("MissionFlowCondition_AND")),
        ]),
    );
    let root = TestInstance::new("MissionFlowCondition_AND").value(
        "conditions",
        TestValue::Array(vec![
            TestValue::Struct(TestInstance::new("MissionFlowCondition_OnMissionStart")),
            TestValue::Struct(alternatives),
        ]),
    );
    archive(root)
}
