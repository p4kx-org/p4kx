// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Synthetic source declarations for resource packaging and owned property states.

use p4kx_export::{BuiltCatalog, CatalogPublicationMode, build_catalog};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::resource_origin_fixture::{self as origin, TestResult};

pub(crate) const RESOURCE: u128 = 0x1069_0001;
pub(crate) const MISSING_BOX: u128 = 0x1069_00ff;

#[derive(Clone, Copy)]
pub(crate) enum Packaging {
    Absent,
    Null,
    Empty,
    Dangling,
}

#[derive(Clone, Copy)]
pub(crate) enum Validation {
    Absent,
    Null,
    False,
}

pub(crate) fn build(
    database: &DataForgeBuilder,
    publication_mode: CatalogPublicationMode,
) -> TestResult<(BuiltCatalog, Value)> {
    let (_directory, archive) = origin::open(database.build(), false)?;
    let mut options = origin::options();
    options.publication_mode = publication_mode;
    let built = build_catalog(&archive, &options)?;
    let catalog = serde_json::from_slice(built.json())?;
    Ok((built, catalog))
}

pub(crate) fn row(catalog: &Value) -> TestResult<&Value> {
    let rows = catalog
        .pointer("/definitions/resourceTypes/rows")
        .and_then(Value::as_array)
        .ok_or("canonical resource rows are missing")?;
    if rows.len() != 1 {
        return Err("the synthetic source must produce exactly one resource row".into());
    }
    rows.first()
        .ok_or_else(|| "the resource row is missing".into())
}

pub(crate) fn packaging(shape: Packaging) -> DataForgeBuilder {
    let resource = TestStruct::new("ResourceType");
    let resource = match shape {
        Packaging::Absent => resource,
        _ => resource.field(
            "defaultCargoContainers",
            TestType::StrongPointer("SResourceTypeDefaultCargoContainers".to_owned()),
        ),
    };
    let value = match shape {
        Packaging::Absent => None,
        Packaging::Null => Some(TestValue::Null),
        Packaging::Empty => Some(TestValue::Struct(TestInstance::new(
            "SResourceTypeDefaultCargoContainers",
        ))),
        Packaging::Dangling => Some(TestValue::Struct(
            TestInstance::new("SResourceTypeDefaultCargoContainers")
                .value("twoSCU", TestValue::Reference(MISSING_BOX))
                .value("one_eighthSCU", TestValue::Null)
                .value("oneSCU", TestValue::Reference(MISSING_BOX)),
        )),
    };
    let record = value.map_or_else(record, |value| {
        record().value("defaultCargoContainers", value)
    });
    DataForgeBuilder::new()
        .version(8)
        .structure(resource)
        .structure(cargo_structure(shape))
        .structure(TestStruct::new("EntityClassDefinition"))
        .record(record)
}

fn cargo_structure(shape: Packaging) -> TestStruct {
    let slots = TestStruct::new("SResourceTypeDefaultCargoContainers");
    match shape {
        Packaging::Dangling => slots
            .field(
                "twoSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "one_eighthSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field(
                "oneSCU",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            ),
        _ => slots,
    }
}

pub(crate) fn validation(shape: Validation) -> DataForgeBuilder {
    let resource = TestStruct::new("ResourceType");
    let (resource, record) = match shape {
        Validation::Absent => (resource, record()),
        // The Bool encoding has no null; a null pointer exercises an explicit source null.
        Validation::Null => (
            resource.field(
                "validateDefaultCargoBox",
                TestType::StrongPointer("FixtureNullableFlag".to_owned()),
            ),
            record().value("validateDefaultCargoBox", TestValue::Null),
        ),
        Validation::False => (
            resource.field("validateDefaultCargoBox", TestType::Bool),
            record().value("validateDefaultCargoBox", TestValue::Bool(false)),
        ),
    };
    DataForgeBuilder::new()
        .version(8)
        .structure(resource)
        .structure(TestStruct::new("FixtureNullableFlag"))
        .record(record)
}

pub(crate) fn unreadable_property() -> DataForgeBuilder {
    // A bad text offset confines the decoder failure to the four-byte property cell.
    properties(
        TestType::String,
        vec![volatility(
            "Readable neighbor",
            TestValue::Raw(u32::MAX.to_le_bytes().to_vec()),
            1.25,
        )],
    )
}

pub(crate) fn nonfinite_properties() -> DataForgeBuilder {
    properties(
        TestType::Single,
        vec![
            volatility(
                "Repeated",
                TestValue::Single(f32::from_bits(0x7fc0_1234)),
                f32::INFINITY,
            ),
            TestValue::Null,
            volatility(
                "Repeated",
                TestValue::Single(f32::NEG_INFINITY),
                f32::from_bits(0xffc0_abcd),
            ),
        ],
    )
}

fn properties(volatility_type: TestType, values: Vec<TestValue>) -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .structure(TestStruct::new("ResourceType").array(
            "properties",
            TestType::StrongPointer("ResourceTypeProperties".to_owned()),
        ))
        .structure(TestStruct::new("ResourceTypeProperties").field("name", TestType::String))
        .structure(
            TestStruct::new("ResourceTypeVolatility")
                .parent("ResourceTypeProperties")
                .field("volatility", volatility_type)
                .field("healthDecayPerSecond", TestType::Single),
        )
        .record(record().value("properties", TestValue::Array(values)))
}

fn volatility(name: &str, value: TestValue, decay: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("ResourceTypeVolatility")
            .value("name", TestValue::text(name))
            .value("volatility", value)
            .value("healthDecayPerSecond", TestValue::Single(decay)),
    )
}

fn record() -> TestRecord {
    TestRecord::new(
        RESOURCE,
        "ResourceType",
        "ResourceType.FixtureOwned",
        "libs/foundry/records/resourcetypedatabase/fixture_owned.xml",
    )
}
