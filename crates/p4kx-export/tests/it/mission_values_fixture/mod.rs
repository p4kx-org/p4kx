// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod investigation;
mod schema;

use crate::mission_property_sets_fixture;
use crate::mission_template_graph::Property;
use crate::scoped_fixture;
use p4kx_export::{CatalogOptions, CatalogPublicationMode, build_catalog};
use p4kx_extraction::Archive;
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntry, TestInstance, TestRecord, TestValue,
};
use serde_json::Value;
use std::error::Error;
use std::time::UNIX_EPOCH;

pub(crate) use investigation::investigation;
pub(crate) type TestResult = Result<(), Box<dyn Error>>;
pub(crate) const TARGET: u128 = 0x7900_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const MISSING: u128 = TARGET + 1;

#[derive(Clone, Copy)]
pub(crate) enum Owner {
    Generator,
    Template,
    Broker,
}

impl Owner {
    pub(crate) fn property<'catalog>(
        self,
        catalog: &'catalog Value,
        variable: &str,
    ) -> Result<Property<'catalog>, Box<dyn Error>> {
        match self {
            Self::Template => Property::new(catalog, variable),
            Self::Broker => Property::broker(catalog, variable),
            Self::Generator => {
                Err("generator values belong to the selected MissionPropertySet".into())
            }
        }
    }
}

pub(crate) const fn object(value: TestInstance) -> TestValue {
    TestValue::Struct(value)
}
pub(crate) const fn array(values: Vec<TestValue>) -> TestValue {
    TestValue::Array(values)
}

pub(crate) fn field<'d>(document: &'d Value, path: &str) -> Result<&'d Value, Box<dyn Error>> {
    let mut current = document;
    let steps = path.trim_start_matches('/').split('/');
    for step in steps {
        current = step
            .parse::<usize>()
            .map_or_else(
                |_| current.as_object().and_then(|fields| fields.get(step)),
                |index| current.as_array().and_then(|values| values.get(index)),
            )
            .ok_or_else(|| format!("missing synthetic field {path}"))?;
    }
    Ok(current)
}
pub(crate) fn property(variable: &str, value: TestInstance) -> TestValue {
    scoped_fixture::property(variable, value)
}
pub(crate) fn tags() -> TestInstance {
    TestInstance::new("TagList").value(
        "tags",
        array(vec![
            TestValue::Reference(TARGET),
            TestValue::Reference(MISSING),
            TestValue::Null,
            TestValue::Reference(TARGET),
        ]),
    )
}

pub(crate) fn database(properties: Vec<TestValue>, owner: Owner) -> DataForgeBuilder {
    let mut builder = schema::add(scoped_fixture::schema(DataForgeBuilder::new().version(8)));
    let generator = match owner {
        Owner::Generator => mission_property_sets_fixture::generator(std::array::from_fn(|_| {
            mission_property_sets_fixture::parameters(properties.clone())
        })),
        Owner::Template => {
            builder = builder.record(
                TestRecord::new(
                    scoped_fixture::TEMPLATE,
                    "ContractTemplate",
                    "ContractTemplate.Values",
                    "libs/foundry/records/contracts/contracttemplates/values.xml",
                )
                .value("contractProperties", array(properties)),
            );
            linked_generator("template", scoped_fixture::TEMPLATE)
        }
        Owner::Broker => {
            builder = builder.record(
                TestRecord::new(
                    scoped_fixture::BROKER,
                    "MissionBrokerEntry",
                    "MissionBrokerEntry.Values",
                    "libs/foundry/records/contracts/values.xml",
                )
                .value("properties", array(properties)),
            );
            linked_generator("missionBrokerEntry", scoped_fixture::BROKER)
        }
    };
    builder.record(generator).record(TestRecord::new(
        TARGET,
        "Definition",
        "Definition.Target",
        "synthetic/definition.xml",
    ))
}

fn linked_generator(field: &str, target: u128) -> TestRecord {
    mission_property_sets_fixture::owner().value(
        "generators",
        array(vec![object(
            TestInstance::new("ContractGeneratorHandler_Career").value(
                "contracts",
                array(vec![object(
                    TestInstance::new("CareerContract")
                        .value("id", TestValue::Guid(TARGET + 5))
                        .value("debugName", TestValue::text("Values"))
                        .value(field, TestValue::Reference(target)),
                )]),
            ),
        )]),
    )
}

pub(crate) fn build(
    builder: &DataForgeBuilder,
    mode: CatalogPublicationMode,
) -> Result<Value, Box<dyn Error>> {
    let root = tempfile::tempdir()?;
    let path = root.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data/Game2.dcb", builder.build()))
        .write_to(&path)?;
    let archive = Archive::open(&path)?;
    let built = build_catalog(
        &archive,
        &CatalogOptions {
            publication_mode: mode,
            ..CatalogOptions::new(UNIX_EPOCH)
        },
    )?;
    Ok(serde_json::from_slice(built.json())?)
}

pub(crate) fn generator_value<'catalog>(
    document: &'catalog Value,
    variable: &str,
) -> Result<&'catalog Value, Box<dyn Error>> {
    let set = mission_property_sets_fixture::level(document, "handler")?;
    mission_property_sets_fixture::property(set, variable)?
        .get("value")
        .ok_or_else(|| "value missing".into())
}
