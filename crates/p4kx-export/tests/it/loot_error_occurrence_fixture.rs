// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};

pub(crate) type Checked<T = ()> = Result<T, Box<dyn Error>>;
pub(crate) const GUID_TEXT: &str = "e9300000-0000-4000-8000-000000000001";
pub(crate) const DANGLING_TEXT: &str = "e9300000-0000-4000-8000-000000000099";
const RECORD_GUID: u128 = 0xe930_0000_0000_4000_8000_0000_0000_0001;
const DANGLING_GUID: u128 = 0xe930_0000_0000_4000_8000_0000_0000_0099;

#[derive(Clone, Copy)]
pub(crate) enum Case {
    RepeatedBool,
    ClassAlias,
    Dangling,
}

#[derive(Clone, Copy)]
pub(crate) enum Storage {
    Plain,
    Encrypted,
}

impl Storage {
    pub(crate) const fn encrypted(self) -> bool {
        matches!(self, Self::Encrypted)
    }
}

pub(crate) struct Fixture {
    pub(crate) archive: Archive,
    _directory: tempfile::TempDir,
}

pub(crate) fn archive(case: Case, storage: Storage) -> Checked<Fixture> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("original-errors.p4k");
    let entry = TestEntry::zstd(r"Data\Game2.dcb", database(case));
    let entry = match storage {
        Storage::Plain => entry,
        Storage::Encrypted => entry.encrypted(),
    };
    ArchiveBuilder::new().entry(entry).write_to(&path)?;
    Ok(Fixture {
        archive: Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?,
        _directory: directory,
    })
}

fn record() -> TestRecord {
    TestRecord::new(
        RECORD_GUID,
        "PoolFilterRecord",
        "PoolFilterRecord.ErrorFacts",
        "libs/foundry/records/lootgeneration/filters/errorfacts.xml",
    )
}

fn database(case: Case) -> Vec<u8> {
    let base = DataForgeBuilder::new().version(8);
    match case {
        Case::RepeatedBool => base
            .structure(TestStruct::new("PoolFilterRecord").array("errors", TestType::Bool))
            .record(record().value(
                "errors",
                TestValue::Array((0..12).map(|_| TestValue::Raw(vec![0x81])).collect()),
            ))
            .build(),
        Case::ClassAlias => class_alias(base),
        Case::Dangling => base
            .structure(TestStruct::new("PoolFilterRecord").field("broken", TestType::Reference))
            .record(record().value("broken", TestValue::Reference(DANGLING_GUID)))
            .build(),
    }
}

fn class_alias(base: DataForgeBuilder) -> Vec<u8> {
    let first = TestInstance::new("Shared").value("number", TestValue::Int32(11));
    base.structure(TestStruct::new("Padding"))
        .structure(TestStruct::new("Shared").field("number", TestType::Int32))
        .structure(
            TestStruct::new("PoolFilterRecord")
                .array("left", TestType::Class("Shared".to_owned()))
                .array("right", TestType::Class("Shared".to_owned())),
        )
        .record(
            record()
                .value("left", TestValue::Array(vec![TestValue::Struct(first)]))
                .value(
                    "right",
                    TestValue::Raw([1_u32.to_le_bytes(), 0_u32.to_le_bytes()].concat()),
                ),
        )
        .build()
}
