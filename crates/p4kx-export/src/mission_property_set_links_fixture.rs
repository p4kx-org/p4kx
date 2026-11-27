// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

pub(super) fn bytes() -> Vec<u8> {
    let mut builder = schema();
    for index in 0..2_u128 {
        builder = builder.record(
            TestRecord::new(
                0xf1b0_0000_0000_4000_8000_0000_0000_0001 + index,
                "Generator",
                format!("Generator.Links{index}"),
                format!("links{index}.xml"),
            )
            .value(
                "generators",
                TestValue::Array(vec![handler(), blank_handler()]),
            ),
        );
    }
    builder.build()
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("Generator")
                .array("generators", TestType::StrongPointer("Handler".to_owned())),
        )
        .structure(
            TestStruct::new("Handler")
                .field(
                    "contractParams",
                    TestType::StrongPointer("Params".to_owned()),
                )
                .array("contracts", TestType::StrongPointer("Contract".to_owned())),
        )
        .structure(
            TestStruct::new("Contract")
                .field(
                    "paramOverrides",
                    TestType::StrongPointer("Params".to_owned()),
                )
                .array("subContracts", TestType::StrongPointer("Sub".to_owned())),
        )
        .structure(
            TestStruct::new("Sub")
                .field(
                    "paramOverrides",
                    TestType::StrongPointer("Params".to_owned()),
                )
                .array("propertyOverrides", TestType::Int32),
        )
        .structure(TestStruct::new("Params"))
        .structure(
            TestStruct::new("GoodParams")
                .parent("Params")
                .array("propertyOverrides", TestType::Int32),
        )
        .structure(
            TestStruct::new("BadParams")
                .parent("Params")
                .field("propertyOverrides", TestType::Int32),
        )
}

fn handler() -> TestValue {
    let first = TestInstance::new("Sub")
        .value("paramOverrides", parameters(3))
        .value(
            "propertyOverrides",
            TestValue::Array(vec![TestValue::Int32(4)]),
        );
    let direct = TestInstance::new("Sub")
        .value("paramOverrides", TestValue::Null)
        .value(
            "propertyOverrides",
            TestValue::Array(vec![TestValue::Int32(5)]),
        );
    let invalid = TestInstance::new("Sub").value(
        "paramOverrides",
        TestValue::Struct(
            TestInstance::new("BadParams").value("propertyOverrides", TestValue::Int32(6)),
        ),
    );
    let contract = TestInstance::new("Contract")
        .value("paramOverrides", parameters(2))
        .value(
            "subContracts",
            TestValue::Array(vec![
                TestValue::Struct(first),
                TestValue::Struct(direct),
                TestValue::Struct(invalid),
            ]),
        );
    TestValue::Struct(
        TestInstance::new("Handler")
            .value("contractParams", parameters(1))
            .value(
                "contracts",
                TestValue::Array(vec![TestValue::Struct(contract)]),
            ),
    )
}

fn blank_handler() -> TestValue {
    TestValue::Struct(TestInstance::new("Handler").value(
        "contracts",
        TestValue::Array(vec![TestValue::Struct(TestInstance::new("Contract"))]),
    ))
}

fn parameters(value: i32) -> TestValue {
    TestValue::Struct(TestInstance::new("GoodParams").value(
        "propertyOverrides",
        TestValue::Array(vec![TestValue::Int32(value)]),
    ))
}
