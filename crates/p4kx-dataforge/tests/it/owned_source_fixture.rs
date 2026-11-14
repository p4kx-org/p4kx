// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;
pub(crate) const NODE: &str = "Node";
pub(crate) const HOLDER: &str = "Holder";

fn name(branch: u32, node: u32) -> String {
    format!("branch-{branch}-node-{node}")
}

pub(crate) fn database(branches: u32, depth: u32) -> Vec<u8> {
    let mut builder = schema();
    for branch in 0..branches {
        for node in 0..depth {
            let next = if node + 1 == depth {
                TestValue::Null
            } else {
                TestValue::pointer(name(branch, node + 1))
            };
            builder = builder.instance(
                name(branch, node),
                TestInstance::new(NODE)
                    .value("mark", TestValue::UInt32(branch * depth + node + 1))
                    .value("next", next),
            );
        }
    }
    let roots = (0..branches)
        .map(|branch| TestValue::pointer(name(branch, 0)))
        .collect();
    builder
        .record(
            TestRecord::new(0x7111, HOLDER, "Holder.Source", "source/owned.xml")
                .value("roots", TestValue::Array(roots))
                .value("tail", TestValue::pointer(name(branches - 1, depth - 1))),
        )
        .build()
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(NODE)
                .field("mark", TestType::UInt32)
                .field("next", TestType::StrongPointer(NODE.to_owned())),
        )
        .structure(
            TestStruct::new(HOLDER)
                .array("roots", TestType::StrongPointer(NODE.to_owned()))
                .field("tail", TestType::WeakPointer(NODE.to_owned())),
        )
}
