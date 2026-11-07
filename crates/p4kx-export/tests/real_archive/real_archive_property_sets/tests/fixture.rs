// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{CatalogOptions, CatalogPublicationMode, ExportHealth, build_catalog_from};
use p4kx_extraction::{Archive, DataForgeEntry};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestStruct, TestType,
    TestValue,
};
use serde_json::Value;

use super::super::TestResult;

const FIRST: u128 = 0x6c00_0000_0000_4000_8000_0000_0000_0001;

pub(super) struct Fixture {
    _directory: tempfile::TempDir,
    pub(super) archive: Archive,
    pub(super) entry: DataForgeEntry,
}

impl Fixture {
    pub(super) fn new() -> TestResult<Self> {
        let directory = tempfile::tempdir()?;
        let path = directory.path().join("Data.p4k");
        let database = schema().record(generator(0)).record(generator(1)).build();
        ArchiveBuilder::new()
            .entry(TestEntry::stored("Data/Game2.dcb", database))
            .write_to(&path)?;
        let archive = Archive::open(&path)?;
        let entry = DataForgeEntry::read(&archive)?;
        Ok(Self {
            _directory: directory,
            archive,
            entry,
        })
    }

    pub(super) fn catalog(&self) -> TestResult<(Value, ExportHealth)> {
        let options = CatalogOptions {
            publication_mode: CatalogPublicationMode::DiagnosticPartial,
            ..CatalogOptions::new(UNIX_EPOCH)
        };
        let built = build_catalog_from(&self.archive, &self.entry, &options)?;
        assert!(
            built.summary().export_health().is_complete(),
            "the source fixture has no actual read or projection failures"
        );
        Ok((
            serde_json::from_slice(built.json())?,
            built.summary().export_health().clone(),
        ))
    }
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn schema() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new("ContractGenerator")
                .array("generators", pointer("ContractGeneratorHandler")),
        )
        .structure(TestStruct::new("ContractGeneratorHandler"))
        .structure(
            TestStruct::new("ContractGeneratorHandler_Career")
                .parent("ContractGeneratorHandler")
                .field("contractParams", pointer("ContractParamOverrides"))
                .array("contracts", TestType::Class("CareerContract".to_owned())),
        )
        .structure(
            TestStruct::new("ContractParamOverrides")
                .array("propertyOverrides", pointer("MissionProperty")),
        )
        .structure(
            TestStruct::new("MissionProperty")
                .field("missionVariableName", TestType::String)
                .field("extendedTextToken", TestType::String)
                .field("value", pointer("BaseMissionPropertyValue")),
        )
        .structure(TestStruct::new("BaseMissionPropertyValue"))
        .structure(
            TestStruct::new("MissionPropertyValue_ScopedLocation")
                .parent("BaseMissionPropertyValue"),
        )
        .structure(
            TestStruct::new("CareerContract")
                .field("id", TestType::Guid)
                .field("debugName", TestType::String),
        )
}

fn generator(index: u128) -> TestRecord {
    let property = TestInstance::new("MissionProperty")
        .value("missionVariableName", TestValue::text("Scoped"))
        .value("extendedTextToken", TestValue::text("Scoped"))
        .value(
            "value",
            TestValue::Struct(TestInstance::new("MissionPropertyValue_ScopedLocation")),
        );
    let parameters = TestInstance::new("ContractParamOverrides").value(
        "propertyOverrides",
        TestValue::Array(vec![TestValue::Struct(property)]),
    );
    let contract = TestInstance::new("CareerContract")
        .value("id", TestValue::Guid(FIRST + 100 + index))
        .value("debugName", TestValue::text("OracleContract"));
    let handler = TestInstance::new("ContractGeneratorHandler_Career")
        .value("contractParams", TestValue::Struct(parameters))
        .value(
            "contracts",
            TestValue::Array(vec![TestValue::Struct(contract)]),
        );
    TestRecord::new(
        FIRST + index,
        "ContractGenerator",
        format!("ContractGenerator.Oracle{index}"),
        format!("libs/foundry/records/contracts/contractgenerator/oracle-{index}.xml"),
    )
    .value(
        "generators",
        TestValue::Array(vec![TestValue::Struct(handler)]),
    )
}
