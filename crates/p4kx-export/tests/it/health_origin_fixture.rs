// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::UNIX_EPOCH;

use p4kx_export::{BuiltCatalog, CatalogOptions, CatalogPublicationMode, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestRecord, TestStruct, TestType, TestValue,
};
use serde_json::Value;

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn Error>>;
pub(crate) const LIST: u128 = 0x5711;
pub(crate) const LIST_TYPE: &str = "BlueprintCategoryDatabaseRecord";
pub(crate) const REGISTRY: &str = "Data\\Libs\\CosmoTools\\LayerRecovery\\LayerBackups.xml";

#[derive(Clone, Copy)]
pub(crate) enum Input {
    Plain,
    Encrypted,
    Standalone,
}
impl Input {
    pub(crate) const fn marker(self) -> Option<bool> {
        match self {
            Self::Plain => Some(false),
            Self::Encrypted => Some(true),
            Self::Standalone => None,
        }
    }
}

pub(crate) struct Fixture {
    pub(crate) folder: tempfile::TempDir,
    pub(crate) archive: Archive,
    pub(crate) entry: DataForgeEntry,
}
impl Fixture {
    pub(crate) fn open(data: &[u8], input: Input, entries: Vec<TestEntry>) -> TestResult<Self> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        let dcb = TestEntry::stored("Data\\Game2.dcb", data);
        let dcb = if matches!(input, Input::Encrypted) {
            dcb.encrypted()
        } else {
            dcb
        };
        let mut archive = ArchiveBuilder::new().entry(dcb);
        for entry in entries {
            archive = archive.entry(entry);
        }
        archive.write_to(&path)?;
        let archive = Archive::open_with_key(path, Some(p4kx_testkit::test_key()))?;
        let entry = if matches!(input, Input::Standalone) {
            let file = folder.path().join("external.dcb");
            std::fs::write(&file, data)?;
            DataForgeEntry::read_file(&file)?
        } else {
            DataForgeEntry::read(&archive)?
        };
        Ok(Self {
            folder,
            archive,
            entry,
        })
    }

    pub(crate) fn diagnostic(&self) -> TestResult<(BuiltCatalog, Value)> {
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..options()
        };
        let built = build_catalog_from(&self.archive, &self.entry, &options)?;
        let catalog = serde_json::from_slice(built.json())?;
        Ok((built, catalog))
    }
}

pub(crate) const fn options() -> CatalogOptions<'static> {
    CatalogOptions::new(UNIX_EPOCH)
}

pub(crate) fn broken_lists(count: u32, empty_file: bool) -> Vec<u8> {
    let mut source = DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(LIST_TYPE)
                .array(
                    "categories",
                    TestType::ReferenceTo("BlueprintCategoryRecord".to_owned()),
                )
                .declared_size(9),
        )
        .structure(TestStruct::new("BlueprintCategoryRecord"));
    for index in 0..count {
        let file = if empty_file {
            String::new()
        } else {
            format!("synthetic/health/List{index}.xml")
        };
        source = source.record(
            TestRecord::new(LIST, LIST_TYPE, format!("{LIST_TYPE}.List{index}"), file)
                .value("categories", TestValue::Array(Vec::new())),
        );
    }
    source.build()
}

pub(crate) fn empty_database() -> Vec<u8> {
    DataForgeBuilder::new().version(8).build()
}

pub(crate) fn guid(value: u128) -> String {
    crate::commodity_v3_fixture::guid(value)
}
