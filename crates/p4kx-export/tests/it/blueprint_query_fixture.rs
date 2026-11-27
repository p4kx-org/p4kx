// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_testkit::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

use crate::crafting_detail_fixture as detail;

pub(crate) const OWNER: u128 = 0x1166_0000_0000_4000_8000_0000_0000_b001;
pub(crate) const CATEGORY: u128 = 0x1166_0000_0000_4000_8000_0000_0000_b002;
pub(crate) const GLOBAL: u128 = 0x1166_0000_0000_4000_8000_0000_0000_b003;
pub(crate) const REPEATED: &str = "RepeatedSlot";
pub(crate) const NAME_KEY: &str = "@sdk_blueprint_name";
const SOURCE: &str = "SdkCraftingBlueprint";
const TIER: &str = "SdkCraftingBlueprintTier";
const RESEARCH: &str = "CraftingResearch";
const COSTS: &str = "CraftingRecipeCosts_Base";
const UNLOCK: &str = "CraftingResearchUnlock_Base";

/// Extends the existing complete crafting source with one physically addressable Blueprint.
pub(crate) fn database() -> DataForgeBuilder {
    declarations(detail::database())
        .record(owner("Primary"))
        .record(TestRecord::new(
            CATEGORY,
            "BlueprintCategoryRecord",
            "BlueprintCategoryRecord.SdkCategory",
            "libs/foundry/records/crafting/blueprintcategories/sdk/category.xml",
        ))
        .record(global())
}

/// Builds another eligible physical declaration of the same external Blueprint GUID.
pub(crate) fn duplicate_owner() -> TestRecord {
    owner("Alias")
}

fn declarations(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new(SOURCE)
                .parent("CraftingBlueprintBase")
                .field("blueprintName", TestType::Locale)
                .field(
                    "category",
                    TestType::ReferenceTo("BlueprintCategoryRecord".to_owned()),
                )
                .field(
                    "processSpecificData",
                    TestType::StrongPointer("CraftingProcess".to_owned()),
                )
                .array("tiers", TestType::Class(TIER.to_owned())),
        )
        .structure(
            TestStruct::new(TIER)
                .field("recipe", TestType::Class("CraftingRecipe".to_owned()))
                .field("research", TestType::StrongPointer(RESEARCH.to_owned())),
        )
        .structure(
            TestStruct::new(RESEARCH)
                .field("researchCosts", TestType::StrongPointer(COSTS.to_owned()))
                .field(
                    "unlockRequirements",
                    TestType::StrongPointer(UNLOCK.to_owned()),
                ),
        )
        .structure(TestStruct::new(COSTS))
        .structure(TestStruct::new(UNLOCK))
        .structure(TestStruct::new("BlueprintCategoryRecord"))
}

fn owner(name: &str) -> TestRecord {
    TestRecord::new(
        OWNER,
        "CraftingBlueprintRecord",
        format!("CraftingBlueprintRecord.BP_CRAFT_Sdk{name}"),
        format!(
            "libs/foundry/records/crafting/blueprints/crafting/sdk/{}.xml",
            name.to_ascii_lowercase()
        ),
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new(SOURCE)
                .value("blueprintName", TestValue::text(NAME_KEY))
                .value("category", TestValue::Reference(CATEGORY))
                .value(
                    "processSpecificData",
                    TestValue::Struct(
                        TestInstance::new("CraftingProcess_Creation")
                            .value("entityClass", TestValue::Reference(detail::PRODUCT_A)),
                    ),
                )
                .value(
                    "tiers",
                    TestValue::Array(vec![tier(TestValue::Null), tier(research())]),
                ),
        ),
    )
}

fn tier(research: TestValue) -> TestValue {
    TestValue::Struct(
        TestInstance::new(TIER)
            .value(
                "recipe",
                TestValue::Struct(TestInstance::new("CraftingRecipe").value("costs", costs())),
            )
            .value("research", research),
    )
}

fn costs() -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingRecipeCosts")
            .value("craftTime", detail::time(1, 15.0))
            .value(
                "mandatoryCost",
                detail::resource(
                    detail::IRON,
                    0.5,
                    vec![
                        detail::multiplier(1.25),
                        detail::inclusion("Include"),
                        detail::modifiers(),
                    ],
                ),
            )
            .value(
                "optionalCosts",
                TestValue::Array(vec![
                    TestValue::Null,
                    detail::select(
                        1,
                        REPEATED,
                        vec![
                            TestValue::Null,
                            detail::resource(detail::GOLD, 0.25, Vec::new()),
                            detail::item(detail::PRODUCT_B, 1),
                        ],
                    ),
                    detail::select(1, REPEATED, vec![detail::item(detail::PRODUCT_B, 2)]),
                ]),
            ),
    )
}

fn research() -> TestValue {
    TestValue::Struct(
        TestInstance::new(RESEARCH)
            .value("researchCosts", TestValue::Struct(TestInstance::new(COSTS)))
            .value("unlockRequirements", TestValue::Null),
    )
}

fn global() -> TestRecord {
    TestRecord::new(
        GLOBAL,
        "CraftingGlobalParams",
        "CraftingGlobalParams.SdkDefaults",
        "libs/foundry/records/crafting/globalparams/sdk/defaults.xml",
    )
    .value(
        "defaultBlueprintSelection",
        TestValue::Struct(
            TestInstance::new("DefaultBlueprintSelection_Whitelist").value(
                "blueprintRecords",
                TestValue::Array(vec![TestValue::Reference(OWNER)]),
            ),
        ),
    )
}
