// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};

pub(crate) const TYPE: &str = "BlueprintCategoryDatabaseRecord";
pub(crate) const CATEGORY_TYPE: &str = "BlueprintCategoryRecord";
pub(crate) const LIST: u128 = 0xf10;
pub(crate) const SECOND: u128 = 0xf20;
pub(crate) const LEFTOVER: u128 = 0xf30;
pub(crate) const ALPHA: u128 = 0x110;
pub(crate) const BETA: u128 = 0x120;
pub(crate) const OTHER: u128 = 0x220;
pub(crate) const MISSING: u128 = 0x990;
pub(crate) const LIST_FILE: &str = "synthetic/category_lists/included/Primary.XML";
pub(crate) const SECOND_FILE: &str = "unrelated/listing/Secondary.XML";
pub(crate) const CATEGORY_FOLDER: &str = "libs/foundry/records/crafting/blueprintcategories/";

pub(crate) fn with_schema(database: DataForgeBuilder) -> DataForgeBuilder {
    database
        .structure(TestStruct::new("Padding"))
        .structure(TestStruct::new(CATEGORY_TYPE))
        .structure(list_schema())
        .structure(TestStruct::new("Other"))
}

pub(crate) fn list_schema() -> TestStruct {
    TestStruct::new(TYPE).array(
        "categories",
        TestType::ReferenceTo(CATEGORY_TYPE.to_owned()),
    )
}

pub(crate) fn listing(guid: u128, name: &str, file: &str, slots: TestValue) -> TestRecord {
    TestRecord::new(guid, TYPE, format!("{TYPE}.{name}"), file)
        .tag(format!("{name}Domain"))
        .value("categories", slots)
}

pub(crate) fn category(guid: u128, name: &str) -> TestRecord {
    TestRecord::new(
        guid,
        CATEGORY_TYPE,
        format!("{CATEGORY_TYPE}.{name}"),
        format!("{CATEGORY_FOLDER}{name}.XML"),
    )
    .tag(format!("{name}Domain"))
}

pub(crate) fn with_clean_lists(database: DataForgeBuilder) -> DataForgeBuilder {
    with_schema(database)
        .record(listing(LIST, "Primary", LIST_FILE, ordered(false)))
        .record(category(ALPHA, "Alpha"))
        .record(TestRecord::new(
            OTHER,
            "Other",
            "Other.Foreign",
            "synthetic/foreign.XML",
        ))
        .record(category(BETA, "Beta"))
        .record(listing(
            SECOND,
            "Secondary",
            SECOND_FILE,
            references(&[ALPHA]),
        ))
        .record(listing(
            LEFTOVER,
            "Test_Listing",
            "synthetic/category_lists/included/Unused.XML",
            references(&[BETA]),
        ))
}

pub(crate) fn ordered(with_errors: bool) -> TestValue {
    let mut slots = vec![
        TestValue::Reference(BETA),
        TestValue::Null,
        TestValue::Reference(ALPHA),
        TestValue::Reference(BETA),
    ];
    if with_errors {
        slots.push(TestValue::Reference(MISSING));
        slots.push(TestValue::Reference(OTHER));
    }
    TestValue::Array(slots)
}

pub(crate) fn references(guids: &[u128]) -> TestValue {
    TestValue::Array(guids.iter().copied().map(TestValue::Reference).collect())
}
