// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Shared synthetic inputs for complete Query selection and demanded read failures.

use crate::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

/// A Station with 4097 tags, an invalid bool, mixed bool-array values and a null owner.
#[must_use]
pub fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .structure(TestStruct::new("Part").field("ready", TestType::Bool))
        .structure(
            TestStruct::new("Station")
                .field("id", TestType::Int32)
                .field("ready", TestType::Bool)
                .field("owner", TestType::StrongPointer("Part".to_owned()))
                .array("tags", TestType::Int32)
                .array("flags", TestType::Bool),
        )
        .structure(TestStruct::new("Other").field("marker", TestType::Int32))
        .record(
            TestRecord::new(1, "Station", "Station.Sample", "synthetic/station.xml")
                .value("id", TestValue::Int32(7))
                .value("ready", TestValue::Raw(vec![2]))
                .value("owner", TestValue::Null)
                .value(
                    "tags",
                    TestValue::Array((0..4097).map(TestValue::Int32).collect()),
                )
                .value(
                    "flags",
                    TestValue::Array(vec![TestValue::Raw(vec![2]), TestValue::Bool(true)]),
                ),
        )
        .record(TestRecord::new(
            2,
            "Other",
            "Other.Sample",
            "synthetic/other.xml",
        ))
}
