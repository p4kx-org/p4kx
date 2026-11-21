// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestInstance, TestRecord, TestStruct, TestType, TestValue,
};

#[path = "catalog_resource_delta_fixture.rs"]
mod delta;

pub(super) const OWNER: u128 = 0x0010_5901;
pub(super) const SPECIALIST: u128 = 0x0010_5902;
pub(super) const RESOURCE: u128 = 0x0010_5903;
pub(super) const MISSING: u128 = 0x0010_5904;
pub(super) const NAN_BITS: u32 = 0x7fc0_1059;

const COMPONENT: &str = "ItemResourceComponentParams";
const STATE: &str = "ItemResourceState";
const REPAIR: &str = "ItemResourceSelfRepairParams";
const RANGES: &str = "PowerRangeParams";
const RANGE: &str = "ResourceRangeParams";
const SIGNATURES: &str = "ItemResourceSignatureParams";
const SIGNATURE: &str = "ItemResourceSignatureEntry";

pub(super) fn database() -> DataForgeBuilder {
    let builder = p4kx_testkit::entity_schema(DataForgeBuilder::new().version(8));
    delta::schema(state_schema(component_schema(builder)))
        .record(
            item(OWNER, "FixtureResourceOnly", "Misc")
                .component(node(component(true)))
                .record(),
        )
        .record(
            item(SPECIALIST, "FixturePowerPlant", "PowerPlant")
                .component(node(component(false)))
                .record(),
        )
        .record(TestRecord::new(
            RESOURCE,
            "ResourceType",
            "ResourceType.Fixture",
            "libs/foundry/records/resourcetypes/fixture_resource.xml",
        ))
}

fn item(guid: u128, name: &str, kind: &str) -> TestEntity {
    TestEntity::new(
        guid,
        name,
        format!("libs/foundry/records/entities/scitem/ships/resource/{name}.xml"),
    )
    .attach_type(kind)
}

fn component_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(COMPONENT)
                .parent("DataForgeComponentParams")
                .field("isResourceNetworked", TestType::Bool)
                .field("isRelay", TestType::Bool)
                .field("isConnectedToRoom", TestType::Bool)
                .field("wirelessConnection", TestType::Bool)
                .field("defaultPriority", TestType::Int32)
                .field("repairRestoreRatio", TestType::Single)
                .field("selfRepair", class(REPAIR))
                .array("states", class(STATE))
                .array("controlBlocks", class("ItemResourceControlBlock")),
        )
        .structure(
            TestStruct::new(REPAIR)
                .field("maxRepairCount", TestType::Int32)
                .field("timeToRepair", TestType::Single)
                .field("healthRatio", TestType::Single),
        )
        .structure(
            TestStruct::new("ItemResourceControlBlock").field("fixtureRule", TestType::Int32),
        )
}

fn state_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(STATE)
                .field("name", TestType::String)
                .array("deltas", pointer(delta::BASE))
                .field("signatureParams", class(SIGNATURES))
                .field("powerRanges", class(RANGES)),
        )
        .structure(
            TestStruct::new(SIGNATURES)
                .field("EMSignature", class(SIGNATURE))
                .field("IRSignature", class(SIGNATURE)),
        )
        .structure(
            TestStruct::new(SIGNATURE)
                .field("nominalSignature", TestType::Single)
                .field("decayRate", TestType::Single),
        )
        .structure(
            TestStruct::new(RANGES)
                .field("low", class(RANGE))
                .field("medium", class(RANGE))
                .field("high", class(RANGE)),
        )
        .structure(
            TestStruct::new(RANGE)
                .field("start", TestType::Int32)
                .field("modifier", TestType::Single)
                .field("registerRange", TestType::Bool),
        )
}

fn component(relay: bool) -> TestInstance {
    let repair = TestInstance::new(REPAIR)
        .value("maxRepairCount", TestValue::Int32(0))
        .value("timeToRepair", TestValue::Single(0.0))
        .value("healthRatio", TestValue::Single(0.25));
    TestInstance::new(COMPONENT)
        .value("isResourceNetworked", TestValue::Bool(true))
        .value("isRelay", TestValue::Bool(relay))
        .value("isConnectedToRoom", TestValue::Bool(false))
        .value("wirelessConnection", TestValue::Bool(false))
        .value("defaultPriority", TestValue::Int32(0))
        .value("repairRestoreRatio", TestValue::Single(-0.0))
        .value("selfRepair", node(repair))
        .value("controlBlocks", TestValue::Array(Vec::new()))
        .value("states", TestValue::Array(vec![node(state())]))
}

fn state() -> TestInstance {
    let signature = TestInstance::new(SIGNATURE)
        .value("nominalSignature", TestValue::Single(0.5))
        .value("decayRate", TestValue::Single(0.0));
    let signatures = TestInstance::new(SIGNATURES).value("EMSignature", node(signature));
    let low = TestInstance::new(RANGE)
        .value("start", TestValue::Int32(0))
        .value("modifier", TestValue::Single(0.0))
        .value("registerRange", TestValue::Bool(false));
    let ranges = TestInstance::new(RANGES).value("low", node(low));
    TestInstance::new(STATE)
        .value("name", TestValue::text("FixtureEnabled"))
        .value("deltas", TestValue::Array(delta::values()))
        .value("signatureParams", node(signatures))
        .value("powerRanges", node(ranges))
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

const fn node(instance: TestInstance) -> TestValue {
    TestValue::Struct(instance)
}
