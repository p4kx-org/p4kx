// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::CatalogPublicationMode;
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestValue};
use serde_json::Value;

use crate::commodity_v3_fixture::{self as fixture, DcbOrigin, TestResult};
use crate::commodity_v3_fixture_schema;

pub(crate) const WINNER: u128 = 0xf0;
const UNREADABLE: u128 = 0x30;

/// The unreadable membership's position relative to a positively selected first group.
#[derive(Clone, Copy)]
pub(crate) enum FaultPosition {
    Absent,
    Before,
    After,
}

pub(crate) fn build(position: FaultPosition) -> TestResult<Value> {
    let partial = !matches!(position, FaultPosition::Absent);
    let database = database(position);
    let (_directory, archive) = fixture::open(&database, DcbOrigin::Plain)?;
    let mut options = fixture::options();
    if partial {
        options.publication_mode = CatalogPublicationMode::DiagnosticPartial;
    }
    let built = p4kx_export::build_catalog(&archive, &options)?;
    Ok(serde_json::from_slice(built.json())?)
}

fn database(position: FaultPosition) -> DataForgeBuilder {
    let builder = commodity_v3_fixture_schema::database()
        .record(
            TestRecord::new(
                fixture::RESOURCE,
                "ResourceType",
                "ResourceType.Fixture",
                "libs/foundry/records/resourcetypedatabase/group_choice.xml",
            )
            .value("displayName", TestValue::text("@resource_name"))
            .value("description", TestValue::text("@resource_description")),
        )
        .record(commodity());
    match position {
        FaultPosition::Absent => builder.record(group(WINNER, fixture::RESOURCE)),
        FaultPosition::Before => builder
            .record(group(UNREADABLE, fixture::DANGLING_RESOURCE))
            .record(group(WINNER, fixture::RESOURCE)),
        FaultPosition::After => builder
            .record(group(WINNER, fixture::RESOURCE))
            .record(group(UNREADABLE, fixture::DANGLING_RESOURCE)),
    }
}

fn commodity() -> TestRecord {
    let composition = TestInstance::new("FixtureComposition")
        .value("entry", TestValue::Reference(fixture::RESOURCE));
    let container = TestInstance::new("ResourceContainer").value(
        "defaultComposition",
        TestValue::Array(vec![TestValue::Struct(composition)]),
    );
    TestRecord::new(
        fixture::ALPHA,
        "EntityClassDefinition",
        "EntityClassDefinition.GroupChoice",
        "libs/foundry/records/entities/commodities/group_choice.xml",
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(container)]),
    )
}

fn group(guid: u128, resource: u128) -> TestRecord {
    TestRecord::new(
        guid,
        "ResourceTypeGroup",
        format!("ResourceTypeGroup.GroupChoice{guid:x}"),
        format!("libs/foundry/records/resourcetypedatabase/group_choice_{guid:x}.xml"),
    )
    .value(
        "resources",
        TestValue::Array(vec![TestValue::Reference(resource)]),
    )
}
