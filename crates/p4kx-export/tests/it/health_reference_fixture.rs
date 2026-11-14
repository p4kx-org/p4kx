// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::{CatalogOptions, CatalogPublicationMode, Filter, build_catalog_from};
use p4kx_testkit::{DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue};
use serde_json::Value;

use crate::health_origin_fixture::{self as fixture, Fixture, Input, TestResult};

pub(crate) const OWNER: u128 = 0x5871;
pub(crate) const TARGET: u128 = 0x5870;
pub(crate) const TYPE: &str = "SCItemManufacturer";
pub(crate) const OWNER_PATH: &str = "libs/foundry/records/scitemmanufacturer/probe";
pub(crate) const LEFT: usize = 17;
pub(crate) const RIGHT: usize = 16;

pub(crate) fn target_path() -> String {
    format!(
        "libs/foundry/records/scitemmanufacturer/{}",
        "m".repeat(8192)
    )
}

pub(crate) fn database() -> Vec<u8> {
    DataForgeBuilder::new()
        .version(8)
        .structure(
            TestStruct::new(TYPE)
                .field("Code", TestType::String)
                .array("Left", TestType::ReferenceTo(TYPE.to_owned()))
                .array("Right", TestType::ReferenceTo(TYPE.to_owned())),
        )
        .record(
            TestRecord::new(
                TARGET,
                TYPE,
                "SCItemManufacturer.Target",
                format!("{}.xml", target_path()),
            )
            .value("Code", TestValue::text("TARGET")),
        )
        .record(
            TestRecord::new(
                OWNER,
                TYPE,
                "SCItemManufacturer.Probe",
                format!("{OWNER_PATH}.xml"),
            )
            .value("Code", TestValue::text("OWNER"))
            .value(
                "Left",
                TestValue::Array(vec![TestValue::Reference(TARGET); LEFT]),
            )
            .value(
                "Right",
                TestValue::Array(vec![TestValue::Reference(TARGET); RIGHT]),
            ),
        )
        .build()
}

pub(crate) fn document(bytes: &[u8], input: Input) -> TestResult<Value> {
    let original = Fixture::open(bytes, input, Vec::new())?;
    let filter = Filter::new("*/probe");
    let options = CatalogOptions {
        filter: Some(&filter),
        publication_mode: CatalogPublicationMode::DiagnosticPartial,
        ..fixture::options()
    };
    let built = build_catalog_from(&original.archive, &original.entry, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}
