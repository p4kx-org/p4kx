// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

pub(crate) const GUID: u128 = 0xe910_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const GUID_TEXT: &str = "e9100000-0000-4000-8000-000000000001";
pub(crate) const PATH: &str = "libs/foundry/records/lootgeneration/filters/retained";
pub(crate) const LITERAL: &str = "  literal\\n@format %%  ";
pub(crate) const LOCALE: &str = "@RawSourceKey";
pub(crate) type Checked<T = ()> = Result<T, Box<dyn Error>>;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    Readable,
    Holes,
}

pub(crate) struct Fixture {
    pub(crate) archive: Archive,
    _directory: tempfile::TempDir,
}

pub(crate) fn archive(case: Case) -> Checked<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data\\Game2.dcb", database(case)))
        .write_to(&path)?;
    Ok(Fixture {
        archive: Archive::open_with_key(path, None)?,
        _directory: directory,
    })
}

fn database(case: Case) -> Vec<u8> {
    schema().record(record(case)).build()
}

fn record(case: Case) -> TestRecord {
    TestRecord::new(
        GUID,
        "PoolFilterRecord",
        "PoolFilterRecord.Retained",
        format!("{PATH}.xml"),
    )
    .value("filter", TestValue::Struct(filter(case)))
    .value("rootExtra", TestValue::UInt64(u64::MAX))
    .value("rootFlag", TestValue::Bool(false))
    .value("rootEmpty", TestValue::text(""))
    .value("rootLocale", TestValue::text(LOCALE))
}

fn filter(case: Case) -> TestInstance {
    let mut slots = vec![instance("first", -7)];
    if matches!(case, Case::Holes) {
        slots.push(TestValue::Null);
        slots.push(TestValue::Raw(
            [77_u32.to_le_bytes(), 99_u32.to_le_bytes()].concat(),
        ));
    }
    slots.push(instance("last", -8));
    TestInstance::new("PoolFilter_Custom")
        .value("filters", TestValue::Array(slots))
        .value(
            "extraArray",
            TestValue::Array(vec![
                TestValue::Int32(11),
                TestValue::Int32(-22),
                TestValue::Int32(0),
            ]),
        )
        .value(
            "extraObject",
            TestValue::Struct(
                TestInstance::new("ExtraPayload")
                    .value("preserveName", TestValue::text(LITERAL))
                    .value("preserveFalse", TestValue::Bool(false)),
            ),
        )
}

fn instance(name: &str, number: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("PoolFilterInstance")
            .value("name", TestValue::text(name))
            .value("mode", TestValue::text("Cumulative"))
            .value(
                "filter",
                TestValue::Struct(
                    TestInstance::new("PoolFilter_Leaf")
                        .value("leafExtra", TestValue::Int32(number)),
                ),
            ),
    )
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("Padding"))
        .structure(TestStruct::new("PoolFilter_Leaf").field("leafExtra", TestType::Int32))
        .structure(
            TestStruct::new("PoolFilterInstance")
                .field("name", TestType::String)
                .field("mode", TestType::String)
                .field("filter", TestType::StrongPointer("PoolFilter_Leaf".into())),
        )
        .structure(
            TestStruct::new("PoolFilter_Custom")
                .array(
                    "filters",
                    TestType::StrongPointer("PoolFilterInstance".into()),
                )
                .array("extraArray", TestType::Int32)
                .field("extraObject", TestType::Class("ExtraPayload".into())),
        )
        .structure(
            TestStruct::new("ExtraPayload")
                .field("preserveName", TestType::String)
                .field("preserveFalse", TestType::Bool),
        )
        .structure(
            TestStruct::new("PoolFilterRecord")
                .field(
                    "filter",
                    TestType::StrongPointer("PoolFilter_Custom".into()),
                )
                .field("rootExtra", TestType::UInt64)
                .field("rootFlag", TestType::Bool)
                .field("rootEmpty", TestType::String)
                .field("rootLocale", TestType::Locale),
        )
}
