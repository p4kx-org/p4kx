// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::commodity_v3_fixture;

/// Healthy optional costs whose distinct selector paths exceed decision-capture storage.
pub(crate) const COST_COUNT: u16 = 1600;
pub(crate) const BLUEPRINT: u128 = 0x104;
pub(crate) const BLUEPRINT_INDEX: u64 = 10;
pub(crate) const INDEPENDENT_RESOURCE: u128 = 4;

pub(crate) fn database() -> DataForgeBuilder {
    structures(commodity_v3_fixture::database())
        .record(independent_resource())
        .record(blueprint())
}

fn structures(builder: DataForgeBuilder) -> DataForgeBuilder {
    [
        TestStruct::new("CraftingBlueprintRecord").field(
            "blueprint",
            TestType::StrongPointer("CraftingBlueprintBase".to_owned()),
        ),
        TestStruct::new("CraftingBlueprintBase"),
        TestStruct::new("CraftingBlueprint")
            .parent("CraftingBlueprintBase")
            .array("tiers", TestType::Class("CraftingBlueprintTier".to_owned())),
        TestStruct::new("CraftingBlueprintTier")
            .field("recipe", TestType::Class("CraftingRecipe".to_owned())),
        TestStruct::new("CraftingRecipe")
            .field("costs", TestType::Class("CraftingRecipeCosts".to_owned())),
        TestStruct::new("CraftingRecipeCosts")
            .field(
                "mandatoryCost",
                TestType::StrongPointer("CraftingCost".to_owned()),
            )
            .array(
                "optionalCosts",
                TestType::StrongPointer("CraftingCost".to_owned()),
            ),
        TestStruct::new("CraftingCost"),
        TestStruct::new("CraftingCost_Resource")
            .parent("CraftingCost")
            .field("minQuality", TestType::Int32)
            .field("resource", TestType::ReferenceTo("ResourceType".to_owned()))
            .field("quantity", TestType::Class("SStandardCargoUnit".to_owned())),
        TestStruct::new("SStandardCargoUnit").field("standardCargoUnits", TestType::Single),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
}

fn independent_resource() -> TestRecord {
    let quantization = TestInstance::new("CraftingQualityQuantization_RecordRef").value(
        "qualityQuantizationRecord",
        TestValue::Reference(commodity_v3_fixture::QUANTIZATION),
    );
    let property = TestInstance::new("ResourceTypeCraftingData")
        .value("name", TestValue::text("Independent"))
        .value("qualityQuantization", TestValue::Struct(quantization));
    TestRecord::new(
        INDEPENDENT_RESOURCE,
        "ResourceType",
        "ResourceType.IndependentCaptureLimit",
        "libs/foundry/records/resourcetypedatabase/independent_capture_limit.xml",
    )
    .value("displayName", TestValue::text("@fallback_resource_name"))
    .value("description", TestValue::text("@resource_description"))
    .value(
        "properties",
        TestValue::Array(vec![TestValue::Struct(property)]),
    )
}

fn blueprint() -> TestRecord {
    let costs = TestInstance::new("CraftingRecipeCosts").value(
        "optionalCosts",
        TestValue::Array((0..COST_COUNT).map(cost).collect()),
    );
    let recipe = TestInstance::new("CraftingRecipe").value("costs", TestValue::Struct(costs));
    let tier =
        TestInstance::new("CraftingBlueprintTier").value("recipe", TestValue::Struct(recipe));
    let blueprint = TestInstance::new("CraftingBlueprint")
        .value("tiers", TestValue::Array(vec![TestValue::Struct(tier)]));
    TestRecord::new(
        BLUEPRINT,
        "CraftingBlueprintRecord",
        "CraftingBlueprintRecord.OriginCaptureLimit",
        "libs/foundry/records/crafting/blueprints/crafting/origin_capture_limit.xml",
    )
    .value("blueprint", TestValue::Struct(blueprint))
}

fn cost(index: u16) -> TestValue {
    let quantity = TestInstance::new("SStandardCargoUnit").value(
        "standardCargoUnits",
        TestValue::Single(f32::from(index) + 1.0),
    );
    TestValue::Struct(
        TestInstance::new("CraftingCost_Resource")
            .value("minQuality", TestValue::Int32(0))
            .value(
                "resource",
                TestValue::Reference(commodity_v3_fixture::RESOURCE_FALLBACK),
            )
            .value("quantity", TestValue::Struct(quantity)),
    )
}
