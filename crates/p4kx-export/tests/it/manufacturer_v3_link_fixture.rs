// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::manufacturer_v3_fixture::{self as fixture, MAKER};

#[derive(Debug, Clone, Copy)]
pub(crate) enum Target {
    Kept,
    Filtered,
    Leftover,
    Outside,
    Alias,
}

pub(crate) fn database(target: Target) -> DataForgeBuilder {
    let path = match target {
        Target::Filtered => "libs/foundry/records/scitemmanufacturer/excluded/maker.xml",
        Target::Leftover => "libs/foundry/records/scitemmanufacturer/test/maker.xml",
        Target::Outside => "libs/foundry/records/elsewhere/maker.xml",
        _ => "libs/foundry/records/scitemmanufacturer/included/maker.xml",
    };
    let database = schema()
        .record(fixture::record(MAKER, "Maker", path).value("Code", TestValue::text("SYN")))
        .record(item());
    if matches!(target, Target::Alias) {
        database.record(fixture::record(
            MAKER,
            "Alias",
            "libs/foundry/records/scitemmanufacturer/included/alias.xml",
        ))
    } else {
        database
    }
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("SCItemManufacturer").field("Code", TestType::String))
        .structure(TestStruct::new("Component"))
        .structure(TestStruct::new("SItemDefinition").field(
            "Manufacturer",
            TestType::ReferenceTo("SCItemManufacturer".to_owned()),
        ))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("Component")
                .field(
                    "AttachDef",
                    TestType::StrongPointer("SItemDefinition".to_owned()),
                ),
        )
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("Component".to_owned()),
        ))
}

fn item() -> TestRecord {
    TestRecord::new(
        0x50,
        "EntityClassDefinition",
        "EntityClassDefinition.IncludedItem",
        "libs/foundry/records/entities/scitem/included/item.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(
            TestInstance::new("SAttachableComponentParams").value(
                "AttachDef",
                TestValue::Struct(
                    TestInstance::new("SItemDefinition")
                        .value("Manufacturer", TestValue::Reference(MAKER)),
                ),
            ),
        )]),
    )
}
