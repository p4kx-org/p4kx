// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const FIRST: u128 = 0xf0;
pub(crate) const CHILD: u128 = 0x10;
pub(crate) const LATER: u128 = 0x20;
const OUTSIDE: u128 = 0x30;
const RESOURCE: u128 = 0x01;
pub(crate) const OTHER: u128 = 0x66;
pub(crate) const MISSING: u128 = 0x99;

pub(crate) fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(group_schema())
        .structure(TestStruct::new("ResourceType"))
        .structure(TestStruct::new("Other"))
}

fn group_schema() -> TestStruct {
    TestStruct::new("ResourceTypeGroup")
        .field("displayName", TestType::Locale)
        .field("description", TestType::Locale)
        .field("defaultThumbnailPath", TestType::String)
        .field("defaultThumbnailPathSVG", TestType::String)
        .array(
            "groups",
            TestType::ReferenceTo("ResourceTypeGroup".to_owned()),
        )
        .array(
            "resources",
            TestType::ReferenceTo("ResourceType".to_owned()),
        )
}

pub(crate) fn references(values: &[u128]) -> TestValue {
    TestValue::Array(values.iter().copied().map(TestValue::Reference).collect())
}

pub(crate) fn group(guid: u128, name: &str, children: TestValue) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceTypeGroup",
        format!("ResourceTypeGroup.{name}"),
        format!("libs/foundry/records/resourcetypedatabase/{name}.xml"),
    )
    .value("displayName", TestValue::text("@shared"))
    .value("description", TestValue::text("@shared"))
    .value("defaultThumbnailPath", TestValue::text(""))
    .value(
        "defaultThumbnailPathSVG",
        TestValue::text("synthetic/icon.svg"),
    )
    .value("groups", children)
    .value("resources", references(&[]))
}

pub(crate) fn graph() -> DataForgeBuilder {
    let slots = TestValue::Array(vec![
        TestValue::Null,
        TestValue::Reference(OTHER),
        TestValue::Reference(MISSING),
        TestValue::Reference(CHILD),
        TestValue::Reference(CHILD),
        TestValue::Reference(FIRST),
    ]);
    schema()
        .record(group(FIRST, "included_first", slots).value(
            "resources",
            TestValue::Array(vec![
                TestValue::Reference(RESOURCE),
                TestValue::Null,
                TestValue::Reference(RESOURCE),
            ]),
        ))
        .record(group(CHILD, "included_child", references(&[FIRST])))
        .record(TestRecord::new(
            OTHER,
            "Other",
            "Other.Wrong",
            "synthetic/other.xml",
        ))
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "ResourceType.Fixture",
            "libs/foundry/records/resourcetypedatabase/included_resource.xml",
        ))
        .record(group(LATER, "included_later", references(&[CHILD])))
        .record(group(CHILD, "included_shadow", references(&[])))
        .record(group(OUTSIDE, "outside", references(&[CHILD])))
}
