// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const TEMPLATE: u128 = 0xf140_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const TEMPLATE_TEXT: &str = "f1400000-0000-4000-8000-000000000001";
const FIRST_PHASE: u128 = 0xf141_0000_0000_4000_8000_0000_0000_0000;
pub(crate) const TRAILING: u128 = 0xf142_0000_0000_4000_8000_0000_0000_0001;

pub(crate) fn id(ordinal: usize) -> TestResult<u128> {
    FIRST_PHASE
        .checked_add(u128::try_from(ordinal)?)
        .ok_or_else(|| "synthetic phase identity overflow".into())
}

pub(crate) fn open(levels: usize, input: Input) -> TestResult<Fixture> {
    let mut children = Vec::new();
    for ordinal in (0..levels).rev() {
        children = vec![TestValue::Struct(
            TestInstance::new("ObjectiveToken")
                .value("id", TestValue::Guid(id(ordinal)?))
                .value("debugName", TestValue::text(format!("phase-{ordinal}")))
                .value("childMissionPhases", TestValue::Array(children)),
        )];
    }
    children.push(TestValue::Struct(
        TestInstance::new("ObjectiveToken")
            .value("id", TestValue::Guid(TRAILING))
            .value("debugName", TestValue::text("trailing")),
    ));
    let database = DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ContractTemplate").array(
            "objectiveTokens",
            TestType::Class("ObjectiveToken".to_owned()),
        ))
        .structure(
            TestStruct::new("ObjectiveToken")
                .field("id", TestType::Guid)
                .field("debugName", TestType::String)
                .array(
                    "childMissionPhases",
                    TestType::Class("ObjectiveToken".to_owned()),
                ),
        )
        .record(
            TestRecord::new(
                TEMPLATE,
                "ContractTemplate",
                "ContractTemplate.CompletePhases",
                "libs/foundry/records/contracts/contracttemplates/complete_phases.xml",
            )
            .value("objectiveTokens", TestValue::Array(children)),
        );
    Fixture::open(&database.build(), input, Vec::new())
}
