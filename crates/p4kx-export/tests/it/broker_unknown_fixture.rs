// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const GUID: u128 = 0xb041_0000_0000_4000_8000_0000_0000_0001;
const BASE_REQUIREMENTS: &str = "BrokerRequirements";
const BASE_TERM: &str = "BrokerStandingTerm";
const BASE_MODIFIER: &str = "MissionModifier";
const REQUIREMENTS: &str = "SReputationMissionRequirementsParams";
const AND: &str = "SReputationMissionRequirementExpression_And";
const LICENSE: &str = "MissionModifier_LawLicense";

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Requirements,
    Term,
    Modifier,
}

impl Case {
    pub(crate) const fn source_type(self) -> &'static str {
        match self {
            Self::Requirements => "SReputationMissionRequirementsUnknown",
            Self::Term => "SReputationMissionRequirementExpression_Unknown",
            Self::Modifier => "MissionModifier_Unknown",
        }
    }
    pub(crate) const fn operation(self) -> &'static str {
        match self {
            Self::Requirements => "reputationRequirements",
            Self::Term => "reputationRequirements expression",
            Self::Modifier => "modifiers",
        }
    }
    pub(crate) const fn ordinal(self) -> u64 {
        match self {
            Self::Requirements => 0,
            Self::Term | Self::Modifier => 3,
        }
    }
    pub(crate) const fn total(self) -> usize {
        match self {
            Self::Requirements => 3,
            Self::Term => 6,
            Self::Modifier => 5,
        }
    }
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}
fn object(name: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(name))
}

fn unknown(case: Case) -> TestValue {
    TestValue::Struct(
        TestInstance::new(case.source_type()).value("originalPayload", TestValue::UInt64(u64::MAX)),
    )
}

pub(crate) fn database(case: Option<Case>) -> Vec<u8> {
    let (requirements, modifiers) = match case {
        Some(Case::Requirements) => (unknown(Case::Requirements), Vec::new()),
        Some(Case::Term) => (
            requirements(vec![object(AND), unknown(Case::Term)]),
            Vec::new(),
        ),
        Some(Case::Modifier) => (
            TestValue::Null,
            vec![object(LICENSE), unknown(Case::Modifier)],
        ),
        None => (requirements(vec![object(AND)]), vec![object(LICENSE)]),
    };
    let mut source = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("MissionBrokerEntry")
                .field("reputationRequirements", pointer(BASE_REQUIREMENTS))
                .array("modifiers", pointer(BASE_MODIFIER)),
        )
        .structure(TestStruct::new(BASE_REQUIREMENTS))
        .structure(
            TestStruct::new(REQUIREMENTS)
                .parent(BASE_REQUIREMENTS)
                .array("expression", pointer(BASE_TERM)),
        )
        .structure(TestStruct::new(BASE_TERM))
        .structure(TestStruct::new(AND).parent(BASE_TERM))
        .structure(TestStruct::new(BASE_MODIFIER))
        .structure(TestStruct::new(LICENSE).parent(BASE_MODIFIER));
    for (case, parent) in [
        (Case::Requirements, BASE_REQUIREMENTS),
        (Case::Term, BASE_TERM),
        (Case::Modifier, BASE_MODIFIER),
    ] {
        source = source.structure(
            TestStruct::new(case.source_type())
                .parent(parent)
                .field("originalPayload", TestType::UInt64),
        );
    }
    source
        .record(
            TestRecord::new(
                GUID,
                "MissionBrokerEntry",
                "MissionBrokerEntry.OriginalGuard",
                "libs/foundry/records/missionbroker/observed/original.xml",
            )
            .value("reputationRequirements", requirements)
            .value("modifiers", TestValue::Array(modifiers)),
        )
        .build()
}

fn requirements(terms: Vec<TestValue>) -> TestValue {
    TestValue::Struct(TestInstance::new(REQUIREMENTS).value("expression", TestValue::Array(terms)))
}
