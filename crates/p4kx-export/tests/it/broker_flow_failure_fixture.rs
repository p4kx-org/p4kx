// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::template_flow_depth_fixture::{self as grammar, Branch, Tail};

pub(crate) const UNKNOWN: &str = "SReputationMissionRequirementsUnknown";
pub(crate) const OPERATION: &str = "reputationRequirements";
pub(crate) const FLOW_VALUES: usize = 22;
pub(crate) const UNKNOWN_ORDINAL: u64 = 22;

pub(crate) fn open(input: Input) -> TestResult<Fixture> {
    let bytes = grammar::schema()
        .structure(
            TestStruct::new("MissionBrokerEntry")
                .field("missionFlow", TestType::Class("MissionFlow".to_owned()))
                .field(
                    "reputationRequirements",
                    TestType::StrongPointer("BrokerRequirements".to_owned()),
                ),
        )
        .structure(TestStruct::new("BrokerRequirements"))
        .structure(
            TestStruct::new(UNKNOWN)
                .parent("BrokerRequirements")
                .field("originalPayload", TestType::UInt64),
        )
        .record(
            TestRecord::new(
                0xb150_0000_0000_4000_8000_0000_0000_0001,
                "MissionBrokerEntry",
                "MissionBrokerEntry.FlowThenUnknown",
                "libs/foundry/records/missionbroker/observed/flow_then_unknown.xml",
            )
            .value(
                "missionFlow",
                grammar::flow(grammar::chain(Branch::Not, 18, Tail::Known)),
            )
            .value(
                "reputationRequirements",
                TestValue::Struct(
                    TestInstance::new(UNKNOWN)
                        .value("originalPayload", TestValue::UInt64(u64::MAX)),
                ),
            ),
        )
        .build();
    Fixture::open(&bytes, input, Vec::new())
}
