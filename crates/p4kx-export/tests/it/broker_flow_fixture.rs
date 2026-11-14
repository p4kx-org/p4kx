// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{TestRecord, TestStruct, TestType};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::template_flow_depth_fixture::{self as grammar, Branch, Tail};

pub(crate) const GUID: &str = "b1500000-0000-4000-8000-000000000001";

pub(crate) fn open(branch: Branch, count: usize, tail: Tail, input: Input) -> TestResult<Fixture> {
    let bytes = grammar::schema()
        .structure(
            TestStruct::new("MissionBrokerEntry")
                .field("missionFlow", TestType::Class("MissionFlow".to_owned())),
        )
        .record(
            TestRecord::new(
                0xb150_0000_0000_4000_8000_0000_0000_0001,
                "MissionBrokerEntry",
                "MissionBrokerEntry.FlatFlow",
                "libs/foundry/records/missionbroker/observed/flat_flow.xml",
            )
            .value(
                "missionFlow",
                grammar::flow(grammar::chain(branch, count, tail)),
            ),
        )
        .build();
    Fixture::open(&bytes, input, Vec::new())
}
