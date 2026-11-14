// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::mission_property_sets_fixture::{generator, parameters};
use crate::scoped_fixture::{object, schema};

#[derive(Clone, Copy)]
pub(crate) enum Case {
    OverrideArray,
    PropertyPointer,
    Empty,
}

impl Case {
    pub(crate) const fn field(self) -> &'static str {
        match self {
            Self::OverrideArray => "propertyOverrides",
            Self::PropertyPointer => "value",
            Self::Empty => "",
        }
    }
}

pub(crate) fn open(case: Case, input: Input) -> TestResult<Fixture> {
    let handler = match case {
        Case::OverrideArray => object(
            TestInstance::new("ContractParamOverrides")
                .value("propertyOverrides", words(1, u32::MAX)),
        ),
        Case::PropertyPointer => parameters(vec![object(
            TestInstance::new("MissionProperty")
                .value("missionVariableName", TestValue::text("Broken"))
                .value("extendedTextToken", TestValue::text("Preserved"))
                .value("value", words(u32::MAX - 1, 3)),
        )]),
        Case::Empty => parameters(Vec::new()),
    };
    let record = generator([handler, parameters(Vec::new()), parameters(Vec::new())]);
    let data = schema(DataForgeBuilder::new().version(8))
        .record(record)
        .build();
    Fixture::open(&data, input, Vec::new())
}

fn words(first: u32, second: u32) -> TestValue {
    TestValue::Raw(
        first
            .to_le_bytes()
            .into_iter()
            .chain(second.to_le_bytes())
            .collect(),
    )
}
