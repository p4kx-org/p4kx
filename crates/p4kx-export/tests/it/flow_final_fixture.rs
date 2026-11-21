// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::flow_final_payload_fixture as payload;
use crate::health_origin_fixture::{Fixture, Input, TestResult};

pub(crate) const GUID: &str = "f1600000-0000-4000-8000-000000000001";
pub(crate) const TARGET: &str = "f1600000-0000-4000-8000-000000000002";
pub(crate) const BASE: &str = "MissionFlowCondition_Base";
pub(crate) const KNOWN: &str = "MissionFlowCondition_OnMissionStart";
pub(crate) const UNKNOWN: &str = "MissionFlowCondition_Mystery";
pub(crate) const TEXT: &str = "  @literal %objective% %.2f M\tline\n\\source  ";
pub(crate) const LOCALE: &str = "@unclassified.flow.key";
pub(crate) const ENUM: &str = "@enum.literal.without.lookup";
pub(crate) const NAN: u32 = 0x7fc0_1234;
const RENAME_FROM: &[u8] = b"missionFloW\0";
const RENAME_TO: &[u8] = b"missionFlow\0";

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Unknown,
    KnownNan,
    OutsideNan,
    Holes,
    Absent,
    Null,
    Empty,
    Duplicate,
}

pub(crate) fn open(case: Case, input: Input) -> TestResult<Fixture> {
    let bytes = database(case)?;
    Fixture::open(&bytes, input, Vec::new())
}

fn database(case: Case) -> TestResult<Vec<u8>> {
    let mut bytes = schema(case)
        .record(record(case))
        .record(TestRecord::new(
            0xf160_0000_0000_4000_8000_0000_0000_0002,
            "ReferenceTarget",
            "ReferenceTarget.Original",
            "synthetic/target.xml",
        ))
        .build();
    if matches!(case, Case::Duplicate) {
        let offsets: Vec<_> = bytes
            .windows(RENAME_FROM.len())
            .enumerate()
            .filter_map(|(index, value)| (value == RENAME_FROM).then_some(index))
            .collect();
        assert_eq!(
            offsets.len(),
            1,
            "one stored second declaration, independently named at write time"
        );
        let start = *offsets.first().ok_or("actual declaration string")?;
        bytes
            .get_mut(start..start + RENAME_FROM.len())
            .ok_or("stored declaration extent")?
            .copy_from_slice(RENAME_TO);
    }
    Ok(bytes)
}

fn schema(case: Case) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration("FlowLiteralEnum", [ENUM])
        .structure(root(case))
        .structure(
            TestStruct::new("MissionFlow")
                .array("triggers", TestType::Class("FlowTrigger".to_owned())),
        )
        .structure(
            TestStruct::new("FlowTrigger")
                .field("condition", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new(BASE))
        .structure(payload::structure(TestStruct::new(KNOWN).parent(BASE)))
        .structure(payload::structure(TestStruct::new(UNKNOWN).parent(BASE)))
        .structure(
            TestStruct::new("MissionFlowCondition_AND")
                .parent(BASE)
                .array("conditions", TestType::StrongPointer(BASE.to_owned())),
        )
        .structure(TestStruct::new("ExtraOwned").field("marker", TestType::Int32))
        .structure(TestStruct::new("ReferenceTarget"))
}

fn root(case: Case) -> TestStruct {
    let mut root = TestStruct::new("ContractTemplate")
        .field("prefix", TestType::UInt32)
        .field(
            "outsideOwned",
            TestType::StrongPointer("ExtraOwned".to_owned()),
        );
    if !matches!(case, Case::Absent) {
        let storage = if matches!(case, Case::Null) {
            TestType::StrongPointer("MissionFlow".to_owned())
        } else {
            TestType::Class("MissionFlow".to_owned())
        };
        root = root.field("missionFlow", storage);
    }
    root = root.field("outsideNumber", TestType::Single);
    if matches!(case, Case::Duplicate) {
        root = root.field("missionFloW", TestType::Class("MissionFlow".to_owned()));
    }
    root
}

fn record(case: Case) -> TestRecord {
    let mut record = original_record();
    if !matches!(case, Case::Absent | Case::Null) {
        record = record.value("missionFlow", TestValue::Struct(flow(case)));
    }
    if matches!(case, Case::OutsideNan) {
        record = record.value("outsideNumber", TestValue::Single(f32::from_bits(NAN)));
    }
    if matches!(case, Case::Duplicate) {
        record = record.value("missionFloW", TestValue::Struct(flow(Case::Unknown)));
    }
    record
}

fn flow(case: Case) -> TestInstance {
    let triggers = if matches!(case, Case::Empty) {
        Vec::new()
    } else {
        vec![TestValue::Struct(
            TestInstance::new("FlowTrigger").value("condition", TestValue::Struct(condition(case))),
        )]
    };
    TestInstance::new("MissionFlow").value("triggers", TestValue::Array(triggers))
}

fn condition(case: Case) -> TestInstance {
    if matches!(case, Case::Holes) {
        return TestInstance::new("MissionFlowCondition_AND").value(
            "conditions",
            TestValue::Array(vec![
                TestValue::Struct(payload::values(KNOWN, false)),
                TestValue::Null,
                TestValue::Raw([77_u32.to_le_bytes(), 99_u32.to_le_bytes()].concat()),
                TestValue::Struct(payload::values(KNOWN, false)),
            ]),
        );
    }
    let name = if matches!(case, Case::Unknown) {
        UNKNOWN
    } else {
        KNOWN
    };
    payload::values(name, matches!(case, Case::KnownNan))
}

fn original_record() -> TestRecord {
    TestRecord::new(
        0xf160_0000_0000_4000_8000_0000_0000_0001,
        "ContractTemplate",
        "ContractTemplate.FinalFlow",
        "libs/foundry/records/contracts/contracttemplates/final_flow.xml",
    )
    .value("prefix", TestValue::UInt32(42))
    .value(
        "outsideOwned",
        TestValue::Struct(
            TestInstance::new("ExtraOwned")
                .named("outside-original")
                .value("marker", TestValue::Int32(11)),
        ),
    )
}
