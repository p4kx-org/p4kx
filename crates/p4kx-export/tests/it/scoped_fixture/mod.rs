// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod schema;

use std::error::Error;
use std::time::{Duration, UNIX_EPOCH};

use p4kx_export::{CatalogOptions, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::Value;

pub(crate) type TestResult = Result<(), Box<dyn Error>>;

pub(crate) fn schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    schema::add(builder)
}

pub(crate) const TEMPLATE: u128 = 0x1100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const BROKER: u128 = 0x2200_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const FOREIGN: u128 = 0x3300_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const DEFINITION_A: u128 = 0x4400_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const DEFINITION_B: u128 = 0x4400_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const SIZE_A: u128 = 0x6600_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const SIZE_B: u128 = 0x6600_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const FUNCTION_A: u128 = 0x9900_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const FUNCTION_B: u128 = 0x9900_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const MISSING: u128 = 0x4400_0000_0000_4000_8000_0000_0000_00ff;

pub(crate) struct Report {
    pub(crate) document: Value,
    pub(crate) failures: Vec<String>,
}

pub(crate) const fn object(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}

pub(crate) fn property(variable: &str, value: TestInstance) -> TestValue {
    object(
        TestInstance::new("MissionProperty")
            .value("missionVariableName", TestValue::text(variable))
            .value("extendedTextToken", TestValue::text(variable))
            .value("value", object(value)),
    )
}

pub(crate) fn scoped(filters: impl IntoIterator<Item = (&'static str, TestValue)>) -> TestInstance {
    filters.into_iter().fold(
        TestInstance::new("MissionPropertyValue_ScopedLocation"),
        |instance, (field, value)| instance.value(field, value),
    )
}

pub(crate) fn build(properties: Vec<TestValue>) -> Result<Report, Box<dyn Error>> {
    build_with_origin(properties, TestValue::text("Origin_BP"))
}

pub(crate) fn build_with_origin(
    properties: Vec<TestValue>,
    origin: TestValue,
) -> Result<Report, Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd(
            "Data\\Game2.dcb",
            database(properties, origin).build(),
        ))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let built = build_catalog(
        &archive,
        &CatalogOptions::new(UNIX_EPOCH + Duration::from_secs(1_792_000_000)),
    )?;
    Ok(Report {
        document: serde_json::from_slice(built.json())?,
        failures: built
            .summary()
            .failures()
            .iter()
            .map(|failure| failure.reason.clone())
            .collect(),
    })
}

fn database(properties: Vec<TestValue>, origin: TestValue) -> DataForgeBuilder {
    let origin = object(
        TestInstance::new("MissionProperty")
            .named("origin")
            .value("missionVariableName", origin)
            .value("extendedTextToken", TestValue::text("Origin"))
            .value(
                "value",
                object(TestInstance::new("MissionPropertyValue_Object")),
            ),
    );
    let properties: Vec<TestValue> = std::iter::once(origin).chain(properties).collect();
    let builder = schema::add(DataForgeBuilder::new().version(8))
        .instance(
            "outside",
            TestInstance::new("MissionProperty")
                .value("missionVariableName", TestValue::text("Outside_BP")),
        )
        .record(
            TestRecord::new(
                FOREIGN,
                "MissionProperty",
                "MissionProperty.Foreign",
                "libs/foundry/records/properties/foreign.xml",
            )
            .value("missionVariableName", TestValue::text("Foreign_BP")),
        )
        .record(template(properties.clone()))
        .record(
            TestRecord::new(
                BROKER,
                "MissionBrokerEntry",
                "MissionBrokerEntry.Scoped",
                "libs/foundry/records/contracts/scoped.xml",
            )
            .value("properties", TestValue::Array(properties)),
        )
        .record(generator());
    [
        ("MissionLocationClass", DEFINITION_A, "A"),
        ("MissionLocationClass", DEFINITION_B, "B"),
        ("MissionLocationSize", SIZE_A, "SizeA"),
        ("MissionLocationSize", SIZE_B, "SizeB"),
        ("MissionLocationFunction", FUNCTION_A, "FunctionA"),
        ("MissionLocationFunction", FUNCTION_B, "FunctionB"),
    ]
    .into_iter()
    .fold(builder, |builder, (kind, guid, name)| {
        builder.record(TestRecord::new(
            guid,
            kind,
            format!("{kind}.{name}"),
            format!("libs/foundry/records/locations/{name}.xml"),
        ))
    })
}

fn template(properties: Vec<TestValue>) -> TestRecord {
    let embedded = property("Embedded_BP", scoped([]));
    let objective = object(
        TestInstance::new("ObjectiveToken")
            .value(
                "id",
                TestValue::Guid(0x7700_0000_0000_4000_8000_0000_0000_0001),
            )
            .value(
                "properties",
                TestValue::Array(vec![object(
                    TestInstance::new("ObjectiveProperty_Embedded").value("property", embedded),
                )]),
            ),
    );
    TestRecord::new(
        TEMPLATE,
        "ContractTemplate",
        "ContractTemplate.Scoped",
        "libs/foundry/records/contracts/contracttemplates/scoped.xml",
    )
    .value(
        "wrongTarget",
        object(
            TestInstance::new("OtherTarget")
                .named("wrong")
                .value("missionVariableName", TestValue::text("MustNotResolve")),
        ),
    )
    .value("contractProperties", TestValue::Array(properties))
    .value("objectiveTokens", TestValue::Array(vec![objective]))
}

fn generator() -> TestRecord {
    let contract = TestInstance::new("CareerContract")
        .value(
            "id",
            TestValue::Guid(0x8800_0000_0000_4000_8000_0000_0000_0001),
        )
        .value("debugName", TestValue::text("ScopedContract"))
        .value("template", TestValue::Reference(TEMPLATE))
        .value("missionBrokerEntry", TestValue::Reference(BROKER));
    TestRecord::new(
        0x5500_0000_0000_4000_8000_0000_0000_0001,
        "ContractGenerator",
        "ContractGenerator.Scoped",
        "libs/foundry/records/contracts/contractgenerator/scoped.xml",
    )
    .value(
        "generators",
        TestValue::Array(vec![object(
            TestInstance::new("ContractGeneratorHandler_Career")
                .value("debugName", TestValue::text("ScopedHandler"))
                .value("contracts", TestValue::Array(vec![object(contract)])),
        )]),
    )
}

pub(crate) fn value<'a>(
    document: &'a Value,
    kind: &str,
    variable: &str,
) -> Result<&'a Value, String> {
    let field = if kind == "contractTemplates" {
        "contractProperties"
    } else {
        "missionProperties"
    };
    document[kind]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|record| record[field].as_array().into_iter().flatten())
        .find(|property| property["variable"] == variable)
        .map(|property| &property["value"])
        .ok_or_else(|| format!("{kind} property {variable}"))
}
