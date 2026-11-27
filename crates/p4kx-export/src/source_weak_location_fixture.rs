// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) const REFERENCES: usize = 64;
/// The owned target struct and its scalar marker precede the weak caller values.
pub(super) const OWNED_VALUES: usize = 2;
const LOCATION_CHARS: usize = 16 << 10;

pub(super) fn location() -> String {
    format!("owned_{}", "x".repeat(LOCATION_CHARS))
}

pub(super) fn database() -> Vec<u8> {
    let name = location();
    let mut structure = TestStruct::new("PoolFilterRecord")
        .field(&name, TestType::StrongPointer("SharedTarget".to_owned()));
    let mut record = TestRecord::new(
        0xe980_0000_0000_4000_8000_0000_0000_0001,
        "PoolFilterRecord",
        "PoolFilterRecord.Locations",
        "owner.xml",
    )
    .value(
        &name,
        TestValue::Struct(
            TestInstance::new("SharedTarget")
                .named("original-shared-target")
                .value("marker", TestValue::Int32(11)),
        ),
    );
    for ordinal in 0..REFERENCES {
        let field = format!("weak{ordinal}");
        structure = structure.field(&field, TestType::WeakPointer("SharedTarget".to_owned()));
        record = record.value(field, TestValue::pointer("original-shared-target"));
    }
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SharedTarget").field("marker", TestType::Int32))
        .structure(structure)
        .record(record)
        .build()
}
