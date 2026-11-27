// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::ExportDiagnosticCode;
use p4kx_testkit::{TestInstance, TestStruct, TestType, TestValue};

use crate::health_origin_fixture::{Fixture, Input, TestResult};
use crate::mission_values_fixture::{self as values, Owner};

pub(crate) const NAN_BITS: u32 = 0x7fc0_1234;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Investigation,
    Name,
    Clue,
    Option,
    Race,
    Organization,
    MissionItem,
    Spawn,
}

impl Case {
    pub(crate) const fn source_type(self) -> &'static str {
        match self {
            Self::Investigation => "Investigation_Suspect",
            Self::Name => "MissionPropertyValue_AIName",
            Self::Clue => "Investigation_Clue_Unknown",
            Self::Option => "MissionPropertyValueOption_Integer",
            Self::Race => "TimeTrialSplit",
            Self::Organization => "DataSetMatchCondition_ExcludeNearbyLocationsDef",
            Self::MissionItem | Self::Spawn => "DataSetMatchCondition_Unknown",
        }
    }

    pub(crate) const fn field(self) -> &'static str {
        match self {
            Self::Investigation => "suspects[0].initialSuspect",
            Self::Name => "suspects[0].name.randomName",
            Self::Clue => "clues",
            Self::Option => "options[0].DEBUG_forceChooseThisOption",
            Self::Race => "targetSplits[0].targetSplit",
            Self::Organization => "matchConditions.exclusionRangeKm",
            Self::MissionItem => "item condition",
            Self::Spawn => "location condition",
        }
    }

    pub(crate) const fn stored_field(self) -> Option<&'static str> {
        match self {
            Self::Investigation => Some("initialSuspect"),
            Self::Name => Some("randomName"),
            Self::Option => Some("DEBUG_forceChooseThisOption"),
            Self::Race => Some("targetSplit"),
            Self::Organization => Some("exclusionRangeKm"),
            Self::Clue | Self::MissionItem | Self::Spawn => None,
        }
    }

    pub(crate) const fn code(self) -> ExportDiagnosticCode {
        match self {
            Self::Investigation | Self::Name | Self::Option => {
                ExportDiagnosticCode::UnreadableSource
            }
            Self::Race | Self::Organization => ExportDiagnosticCode::MalformedSource,
            Self::Clue | Self::MissionItem | Self::Spawn => ExportDiagnosticCode::UnsupportedValue,
        }
    }
}

pub(crate) fn open(case: Case, input: Input) -> TestResult<Fixture> {
    open_owner(case, Owner::Generator, input)
}

pub(crate) fn open_broker(case: Case, input: Input) -> TestResult<Fixture> {
    open_owner(case, Owner::Broker, input)
}

fn open_owner(case: Case, owner: Owner, input: Input) -> TestResult<Fixture> {
    let value = property(case);
    let data = values::database(vec![values::property("Original", value)], owner)
        .structure(
            TestStruct::new("MissionPropertyValue_MissionItem")
                .parent("BaseMissionPropertyValue")
                .array(
                    "matchConditions",
                    TestType::StrongPointer("DataSetMatchCondition".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("MissionPropertyValue_Location")
                .parent("BaseMissionPropertyValue")
                .array(
                    "matchConditions",
                    TestType::StrongPointer("DataSetMatchCondition".to_owned()),
                ),
        )
        .structure(TestStruct::new("DataSetMatchCondition_Unknown").parent("DataSetMatchCondition"))
        .build();
    Fixture::open(&data, input, Vec::new())
}

fn property(case: Case) -> TestInstance {
    match case {
        Case::Investigation => suspect(
            TestInstance::new("Investigation_Suspect")
                .value("initialSuspect", TestValue::Raw(vec![3])),
        ),
        Case::Name => suspect(
            TestInstance::new("Investigation_Suspect").value(
                "name",
                values::object(
                    TestInstance::new("MissionPropertyValue_AIName")
                        .value("randomName", TestValue::Raw(vec![3])),
                ),
            ),
        ),
        Case::Clue => clue(),
        Case::Option => option(),
        Case::Race => TestInstance::new("MissionPropertyValue_TimeTrialRace").value(
            "targetSplits",
            values::array(vec![values::object(
                TestInstance::new("TimeTrialSplit").value("targetSplit", nan()),
            )]),
        ),
        Case::Organization => condition(
            "Organization",
            TestInstance::new("DataSetMatchCondition_ExcludeNearbyLocationsDef")
                .value("exclusionRangeKm", nan()),
        ),
        Case::MissionItem => condition(
            "MissionItem",
            TestInstance::new("DataSetMatchCondition_Unknown"),
        ),
        Case::Spawn => condition(
            "Location",
            TestInstance::new("DataSetMatchCondition_Unknown"),
        ),
    }
}

fn clue() -> TestInstance {
    let clue = TestInstance::new("Investigation_Clue_Unknown");
    let item = TestInstance::new("Investigation_ClueItem")
        .value("clues", values::array(vec![values::object(clue)]));
    let location = TestInstance::new("Investigation_Location")
        .value("clueItems", values::array(vec![values::object(item)]));
    TestInstance::new("MissionPropertyValue_Investigation")
        .value("locations", values::array(vec![values::object(location)]))
}

fn option() -> TestInstance {
    let option = TestInstance::new("MissionPropertyValueOption_Integer")
        .value("value", TestValue::Int32(7))
        .value("DEBUG_forceChooseThisOption", TestValue::Raw(vec![3]));
    TestInstance::new("MissionPropertyValue_Integer")
        .value("options", values::array(vec![values::object(option)]))
}

fn suspect(value: TestInstance) -> TestInstance {
    TestInstance::new("MissionPropertyValue_Investigation")
        .value("suspects", values::array(vec![values::object(value)]))
}

fn condition(kind: &str, value: TestInstance) -> TestInstance {
    TestInstance::new(format!("MissionPropertyValue_{kind}")).value(
        "matchConditions",
        values::array(vec![values::object(value)]),
    )
}

const fn nan() -> TestValue {
    TestValue::Single(f32::from_bits(NAN_BITS))
}
