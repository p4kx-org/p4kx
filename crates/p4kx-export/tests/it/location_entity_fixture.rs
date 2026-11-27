// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::{LocationEntityDeclaration, SourcePayload};
use p4kx_extraction::{InputScope, SourceInventory, SourceLocation, SourceUseRole};
use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::commodity_v3_fixture::TestResult;

const SOURCE: &str = "LocationEntityDeclaration";
const BASE: &str = "LocationEntityType_Base";
const DELIVERY: &str = "LocationEntityType_DeliveryTarget";
const STATIC: &str = "LocationEntityType_StaticEntity";
const FUTURE: &str = "FixtureLocationEntityType";
const DELIVERY_ENUM: &str = "DeliveryObjectiveType";
const SLOT: &str = "LocationResourceSlot";
pub(super) const DUPLICATE: u128 = 0x0011_7101;
const DELIVERY_OWNER: u128 = 0x0011_7102;
const TAG: u128 = 0x0011_7103;
const SLOT_RECORD: u128 = 0x0011_7104;
pub(super) const MISSING: u128 = 0x0011_7105;
pub(super) const GENERATOR: u128 = 0x0011_7106;

pub(super) fn database() -> DataForgeBuilder {
    DataForgeBuilder::new()
        .version(8)
        .enumeration(DELIVERY_ENUM, ["FixtureDelivery"])
        .structure(TestStruct::new("Tag"))
        .structure(tag_list())
        .structure(TestStruct::new(SLOT))
        .structure(TestStruct::new(BASE))
        .structure(TestStruct::new(STATIC).parent(BASE))
        .structure(
            TestStruct::new(FUTURE)
                .parent(BASE)
                .field("unmodeled", TestType::Bool),
        )
        .structure(
            TestStruct::new(DELIVERY)
                .parent(BASE)
                .field("type", TestType::Enum(DELIVERY_ENUM.to_owned()))
                .field("targetDescription", TestType::Locale),
        )
        .structure(
            TestStruct::new(SOURCE)
                .field("entityTags", TestType::Class("TagList".to_owned()))
                .field("resourceSlot", reference(SLOT))
                .field("type", TestType::StrongPointer(BASE.to_owned())),
        )
        .record(TestRecord::new(TAG, "Tag", "FixtureTag", "fixture/tag.xml"))
        .record(TestRecord::new(
            SLOT_RECORD,
            SLOT,
            "FixtureResourceSlot",
            "fixture/slot.xml",
        ))
        .record(declaration(
            DUPLICATE,
            "fixture_static",
            TestInstance::new(STATIC),
            Vec::new(),
        ))
        .record(declaration(
            DUPLICATE,
            "fixture_future",
            TestInstance::new(FUTURE).value("unmodeled", TestValue::Bool(true)),
            Vec::new(),
        ))
        .record(declaration(
            DELIVERY_OWNER,
            "fixture_delivery",
            TestInstance::new(DELIVERY)
                .value("type", TestValue::text("FixtureDelivery"))
                .value("targetDescription", TestValue::text("@resource_name")),
            vec![
                TestValue::Reference(TAG),
                TestValue::Null,
                TestValue::Reference(TAG),
                TestValue::Reference(MISSING),
            ],
        ))
}

fn tag_list() -> TestStruct {
    TestStruct::new("TagList")
        .array("tags", reference("Tag"))
        .field("fixtureFlag", TestType::Bool)
}

fn declaration(guid: u128, name: &str, ty: TestInstance, tags: Vec<TestValue>) -> TestRecord {
    TestRecord::new(guid, SOURCE, name, format!("fixture/{name}.xml"))
        .value(
            "entityTags",
            TestValue::Struct(
                TestInstance::new("TagList")
                    .value("tags", TestValue::Array(tags))
                    .value("fixtureFlag", TestValue::Bool(true)),
            ),
        )
        .value("type", TestValue::Struct(ty))
        .value("resourceSlot", TestValue::Reference(SLOT_RECORD))
}

pub(super) fn verify_authored_tag_field(
    row: &LocationEntityDeclaration<'_>,
    inventory: &SourceInventory,
) -> TestResult {
    let tags = row.entity_tags().value().ok_or("the original TagList")?;
    let arena = tags.authored_fields();
    let [field] = arena.roots() else {
        return Err("one original member outside the typed tags array".into());
    };
    assert_eq!(
        field.name().value(),
        Some(&"fixtureFlag"),
        "the SDK keeps the original field name"
    );
    let cell = arena.value(field.value()).ok_or("the flag value")?.value();
    assert!(
        matches!(cell.value(), Some(SourcePayload::Boolean { value: true })),
        "the SDK resolves the original Boolean without serializing the row"
    );
    let origin = cell.origin();
    assert_eq!(
        origin.contains_encrypted_source(),
        Some(false),
        "the flag has its own complete plain DataForge origin"
    );
    let [use_id] = origin.source_uses_refs() else {
        return Err("one original flag payload use".into());
    };
    let source_use = inventory
        .iter_uses()
        .find(|source_use| source_use.id() == use_id)
        .ok_or("the SDK inventory retains the original flag source use")?;
    let record_index = row
        .source()
        .record_index()
        .value()
        .copied()
        .ok_or("the original physical Record index")?;
    assert!(
        source_use.scope() == InputScope::Payload
            && source_use.role() == SourceUseRole::Payload
            && matches!(source_use.location(),
                SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeValue"
                    && indices.first() == Some(&u64::from(record_index))
                    && field == "$value"),
        "the use addresses the original value on the selected physical Record"
    );
    Ok(())
}

fn reference(target: &str) -> TestType {
    TestType::ReferenceTo(target.to_owned())
}

const GENERATOR_TYPE: &str = "ContractGenerator";
const HAULING_HANDLER: &str = "FixtureHaulingHandler";
const HAULING_CONTRACT: &str = "FixtureHaulingContract";
const HAULING_SUB_CONTRACT: &str = "FixtureHaulingSubContract";
const HAULING_PARAMS: &str = "FixtureHaulingOverrides";
const HAULING_PROPERTY: &str = "FixtureHaulingProperty";
const HAULING_VALUE: &str = "MissionPropertyValue_HaulingOrders";
const HAULING_ORDER: &str = "HaulingOrder_Fixture";

pub(super) fn hauling_database() -> DataForgeBuilder {
    let sub = TestInstance::new(HAULING_SUB_CONTRACT).value(
        "paramOverrides",
        hauling_override(Vec::new(), vec![TestValue::Reference(DELIVERY_OWNER)]),
    );
    let contract = TestInstance::new(HAULING_CONTRACT)
        .value(
            "paramOverrides",
            hauling_override(
                vec![
                    TestValue::Reference(DELIVERY_OWNER),
                    TestValue::Null,
                    TestValue::Reference(DELIVERY_OWNER),
                ],
                Vec::new(),
            ),
        )
        .value(
            "subContracts",
            TestValue::Array(vec![TestValue::Struct(sub)]),
        );
    let handler = TestInstance::new(HAULING_HANDLER).value(
        "contracts",
        TestValue::Array(vec![TestValue::Struct(contract)]),
    );
    hauling_structures()
        .into_iter()
        .fold(database(), DataForgeBuilder::structure)
        .record(
            TestRecord::new(
                GENERATOR,
                GENERATOR_TYPE,
                "FixtureHauling",
                "libs/foundry/records/contracts/contractgenerator/fixture_hauling.xml",
            )
            .value(
                "generators",
                TestValue::Array(vec![TestValue::Struct(handler)]),
            ),
        )
}

fn hauling_structures() -> [TestStruct; 8] {
    [
        TestStruct::new(GENERATOR_TYPE).array("generators", pointer(HAULING_HANDLER)),
        TestStruct::new(HAULING_HANDLER).array("contracts", pointer(HAULING_CONTRACT)),
        TestStruct::new(HAULING_CONTRACT)
            .field("paramOverrides", pointer(HAULING_PARAMS))
            .array("subContracts", pointer(HAULING_SUB_CONTRACT)),
        TestStruct::new(HAULING_SUB_CONTRACT).field("paramOverrides", pointer(HAULING_PARAMS)),
        TestStruct::new(HAULING_PARAMS).array("propertyOverrides", pointer(HAULING_PROPERTY)),
        TestStruct::new(HAULING_PROPERTY).field("value", pointer(HAULING_VALUE)),
        TestStruct::new(HAULING_VALUE).array("haulingOrderContent", pointer(HAULING_ORDER)),
        TestStruct::new(HAULING_ORDER)
            .array("pickUpTargetTypes", reference(SOURCE))
            .array("dropOffTargetTypes", reference(SOURCE)),
    ]
}

fn hauling_override(pickup: Vec<TestValue>, dropoff: Vec<TestValue>) -> TestValue {
    let order = TestInstance::new(HAULING_ORDER)
        .value("pickUpTargetTypes", TestValue::Array(pickup))
        .value("dropOffTargetTypes", TestValue::Array(dropoff));
    let value = TestInstance::new(HAULING_VALUE).value(
        "haulingOrderContent",
        TestValue::Array(vec![TestValue::Struct(order)]),
    );
    let property = TestInstance::new(HAULING_PROPERTY).value("value", TestValue::Struct(value));
    TestValue::Struct(TestInstance::new(HAULING_PARAMS).value(
        "propertyOverrides",
        TestValue::Array(vec![TestValue::Struct(property)]),
    ))
}

fn pointer(target: &str) -> TestType {
    TestType::StrongPointer(target.to_owned())
}
