// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const BEFORE: &str = "f1900000-0000-4000-8000-000000000011";
pub(super) const AFTER: &str = "f1900000-0000-4000-8000-000000000022";
const FROM: &[u8] = b"targeT\0";
const TO: &[u8] = b"target\0";

#[derive(Clone, Copy)]
pub(super) enum Values {
    Distinct,
    Equal,
}

pub(super) fn database(values: Values) -> Vec<u8> {
    let mut bytes = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Owner")
                .field("target", TestType::StrongPointer("Target".to_owned()))
                .field("forward", TestType::WeakPointer("Target".to_owned()))
                .array("targets", TestType::StrongPointer("Target".to_owned()))
                .field("backward", TestType::WeakPointer("Target".to_owned()))
                .field("targeT", TestType::StrongPointer("Target".to_owned()))
                .field("arrayTarget", TestType::WeakPointer("Target".to_owned())),
        )
        .structure(
            TestStruct::new("Target")
                .field("id", TestType::Guid)
                .field("missionVariableName", TestType::String),
        )
        .record(record(values))
        .build();
    let starts: Vec<_> = bytes
        .windows(FROM.len())
        .enumerate()
        .filter_map(|(index, bytes)| (bytes == FROM).then_some(index))
        .collect();
    assert_eq!(starts.len(), 1, "one genuine later declaration spelling");
    let start = *starts.first().expect("the exact stored declaration");
    bytes
        .get_mut(start..start + FROM.len())
        .expect("the original declaration bytes")
        .copy_from_slice(TO);
    bytes
}

fn record(values: Values) -> TestRecord {
    TestRecord::new(
        0xf190_0000_0000_4000_8000_0000_0000_0001,
        "Owner",
        "Owner.Positions",
        "owner.xml",
    )
    .value("target", TestValue::Struct(target("before", false, values)))
    .value("targeT", TestValue::Struct(target("after", true, values)))
    .value(
        "targets",
        TestValue::Array(vec![
            TestValue::Struct(target("array-before", false, values)),
            TestValue::Null,
            TestValue::Raw([77_u32.to_le_bytes(), 99_u32.to_le_bytes()].concat()),
            TestValue::Struct(target("array-after", true, values)),
        ]),
    )
    .value("forward", TestValue::pointer("after"))
    .value("backward", TestValue::pointer("before"))
    .value("arrayTarget", TestValue::pointer("array-after"))
}

fn target(name: &str, later: bool, values: Values) -> TestInstance {
    let later = later && matches!(values, Values::Distinct);
    TestInstance::new("Target")
        .named(name)
        .value(
            "id",
            TestValue::Guid(if later {
                0xf190_0000_0000_4000_8000_0000_0000_0022
            } else {
                0xf190_0000_0000_4000_8000_0000_0000_0011
            }),
        )
        .value(
            "missionVariableName",
            TestValue::text(if later {
                " after-original "
            } else {
                "before-original"
            }),
        )
}
