// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::gameplay_v3_schema as schema;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};

pub(crate) const FIRST: u128 = 0xc201;
pub(crate) const SECOND: u128 = 0xc202;
pub(crate) const FOLDER: &str = "libs/foundry/records/crafting/craftedproperties/";
pub(crate) const FIRST_FILE: &str = "libs/foundry/records/crafting/craftedproperties/Primary.XML";

pub(crate) fn record(guid: u128, name: &str, display: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        schema::PROPERTY,
        format!("{}.{name}", schema::PROPERTY),
        format!("{FOLDER}{name}.XML"),
    )
    .tag("SyntheticCrafting")
    .value("propertyName", TestValue::text("@resource_name"))
    .value("displayTransformation", display)
    .value("unitFormat", TestValue::text("@own_type"))
    .value("nameOverrides", TestValue::Array(Vec::new()))
}

pub(crate) fn owned(name: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(name))
}

pub(crate) fn scale(value: TestValue) -> TestValue {
    TestValue::Struct(TestInstance::new(schema::SCALE).value("scale", value))
}

pub(crate) fn sequence(values: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new(schema::SEQUENCE).value("transformations", TestValue::Array(values)),
    )
}

pub(crate) fn name_override(key: &str, condition: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(schema::OVERRIDE)
            .value("propertyName", TestValue::text(key))
            .value("condition", condition),
    )
}

pub(crate) fn item_type(items: Vec<TestValue>, subtypes: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new(schema::ITEM_TYPE)
            .value("matchItemTypes", TestValue::Array(items))
            .value("matchSubTypes", TestValue::Array(subtypes)),
    )
}

pub(crate) fn simple() -> DataForgeBuilder {
    schema::schema().record(record(FIRST, "Primary", scale(TestValue::Single(1.25))))
}

pub(crate) fn recursive() -> TestValue {
    sequence(vec![
        owned(schema::PERCENT),
        TestValue::Null,
        sequence(vec![scale(TestValue::Single(-0.0)), owned(schema::PERCENT)]),
    ])
}
