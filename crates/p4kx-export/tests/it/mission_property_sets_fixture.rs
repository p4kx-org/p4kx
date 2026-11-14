// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::{Archive, Filter};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::Value;

use crate::scoped_fixture::{self, Report, object};

pub(crate) const GENERATOR: &str = "55000000-0000-4000-8000-000000000001";

pub(crate) fn parameters(properties: Vec<TestValue>) -> TestValue {
    object(
        TestInstance::new("ContractParamOverrides")
            .value("propertyOverrides", TestValue::Array(properties)),
    )
}

pub(crate) fn generator(parameters: [TestValue; 3]) -> TestRecord {
    let [handler, contract, sub] = parameters;
    with_sub(
        [handler, contract],
        &TestInstance::new("SubContract").value("paramOverrides", sub),
    )
}

pub(crate) fn with_sub(parameters: [TestValue; 2], sub: &TestInstance) -> TestRecord {
    let [handler, contract] = parameters;
    let subs = (1_u128..=2).map(|id| {
        object(
            sub.clone()
                .value(
                    "id",
                    TestValue::Guid(0xaa00_0000_0000_4000_8000_0000_0000_0000 + id),
                )
                .value("debugName", TestValue::text(format!("Sub{id}"))),
        )
    });
    let contract = TestInstance::new("CareerContract")
        .value(
            "id",
            TestValue::Guid(0x8800_0000_0000_4000_8000_0000_0000_0001),
        )
        .value("debugName", TestValue::text("ScopedContract"))
        .value("paramOverrides", contract)
        .value("subContracts", TestValue::Array(subs.collect()));
    owner().value(
        "generators",
        TestValue::Array(vec![object(
            TestInstance::new("ContractGeneratorHandler_Career")
                .value("debugName", TestValue::text("ScopedHandler"))
                .value("contractParams", handler)
                .value("contracts", TestValue::Array(vec![object(contract)])),
        )]),
    )
}

pub(crate) fn owner() -> TestRecord {
    TestRecord::new(
        0x5500_0000_0000_4000_8000_0000_0000_0001,
        "ContractGenerator",
        "ContractGenerator.ScopedSource",
        "libs/foundry/records/contracts/contractgenerator/scoped-source.xml",
    )
}

pub(crate) fn build(record: TestRecord, filter: Option<&str>) -> Result<Report, Box<dyn Error>> {
    build_mode(record, filter, CatalogPublicationMode::Normal)
}

pub(crate) fn diagnostic(
    record: TestRecord,
    filter: Option<&str>,
) -> Result<Report, Box<dyn Error>> {
    build_mode(record, filter, CatalogPublicationMode::DiagnosticPartial)
}

fn build_mode(
    record: TestRecord,
    filter: Option<&str>,
    publication_mode: CatalogPublicationMode,
) -> Result<Report, Box<dyn Error>> {
    let builder = scoped_fixture::schema(DataForgeBuilder::new().version(8))
        .record(record)
        .record(TestRecord::new(
            scoped_fixture::DEFINITION_A,
            "MissionLocationClass",
            "MissionLocationClass.A",
            "libs/foundry/records/locations/class-a.xml",
        ))
        .record(TestRecord::new(
            scoped_fixture::SIZE_A,
            "MissionLocationSize",
            "MissionLocationSize.A",
            "libs/foundry/records/locations/size-a.xml",
        ))
        .record(TestRecord::new(
            scoped_fixture::FUNCTION_A,
            "MissionLocationFunction",
            "MissionLocationFunction.A",
            "libs/foundry/records/locations/function-a.xml",
        ))
        .record(
            TestRecord::new(
                scoped_fixture::FOREIGN,
                "MissionProperty",
                "MissionProperty.Foreign",
                "libs/foundry/records/properties/foreign.xml",
            )
            .value("missionVariableName", TestValue::text("Foreign_BP")),
        );
    build_database(&builder, filter, publication_mode)
}

fn build_database(
    builder: &DataForgeBuilder,
    filter: Option<&str>,
    publication_mode: CatalogPublicationMode,
) -> Result<Report, Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", builder.build()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let filter = filter.map(Filter::new);
    let options = CatalogOptions {
        publication_mode,
        filter: filter.as_ref(),
        ..CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000))
    };
    let built = build_catalog(&archive, &options)?;
    Ok(Report {
        document: serde_json::from_slice(built.json())?,
        failures: built
            .summary()
            .failures()
            .iter()
            .map(|failure| failure.reason().to_owned())
            .collect(),
    })
}

pub(crate) fn rows(document: &Value) -> Result<&Vec<Value>, String> {
    document["missionPropertySets"]
        .as_array()
        .ok_or_else(|| "missionPropertySets".to_owned())
}

pub(crate) fn level<'a>(document: &'a Value, level: &str) -> Result<&'a Value, String> {
    rows(document)?
        .iter()
        .find(|set| set["level"] == level)
        .ok_or_else(|| format!("set {level}"))
}

pub(crate) fn property<'a>(set: &'a Value, variable: &str) -> Result<&'a Value, String> {
    set["properties"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|property| property["variable"] == variable)
        .ok_or_else(|| format!("property {variable}"))
}
