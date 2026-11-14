// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with commodities and crafting blueprints for the P4KX catalog tests.

use std::error::Error;
use std::path::{Path, PathBuf};

mod commodities;
mod structures;

use commodities::{commodity_records, prop};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestValue,
};

pub(crate) const GOLD: u128 = 0xa100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const DOPPLE: u128 = 0xa200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const RESOURCE_GOLD: u128 = 0xb100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const RESOURCE_IRON: u128 = 0xb200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const RESOURCE_EMBER: u128 = 0xb300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const RESOURCE_COPPER: u128 = 0xb400_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const GROUP_METAL: u128 = 0xc100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const GROUP_MINERAL: u128 = 0xc200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const GROUP_ALLOY: u128 = 0xc300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const CATEGORY: u128 = 0xd100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CATEGORY_ARMOUR: u128 = 0xd200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const CATEGORY_LEFTOVER: u128 = 0xd300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const CATEGORY_DATABASE: u128 = 0xd400_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const PRODUCT: u128 = 0xe100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const ORE: u128 = 0xe200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const MISSING: u128 = 0xe300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const MAGAZINE: u128 = 0x1100_0000_0000_4000_8000_0000_0000_0001;
pub(crate) const CORE: u128 = 0x1200_0000_0000_4000_8000_0000_0000_0002;
pub(crate) const DANGLING: u128 = 0x1300_0000_0000_4000_8000_0000_0000_0003;
pub(crate) const TEMPLATE: u128 = 0x1400_0000_0000_4000_8000_0000_0000_0004;
pub(crate) const NESTED: u128 = 0x1700_0000_0000_4000_8000_0000_0000_0007;

pub(crate) fn resource(guid: u128, scu: f32, quality: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCost_Resource")
            .value("minQuality", TestValue::Int32(quality))
            .value("resource", TestValue::Reference(guid))
            .value(
                "quantity",
                TestValue::Struct(
                    TestInstance::new("SStandardCargoUnit")
                        .value("standardCargoUnits", TestValue::Single(scu)),
                ),
            ),
    )
}

pub(crate) fn item(guid: u128, units: i32, quality: i32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCost_Item")
            .value("quantity", TestValue::Int32(units))
            .value("minQuality", TestValue::Int32(quality))
            .value("entityClass", TestValue::Reference(guid)),
    )
}

pub(crate) fn select(debug: &str, key: &str, options: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("CraftingCost_Select")
            .value(
                "nameInfo",
                TestValue::Struct(
                    TestInstance::new("CraftingNameInfo")
                        .value("debugName", TestValue::text(debug))
                        .value("displayName", TestValue::text(key)),
                ),
            )
            .value("options", TestValue::Array(options)),
    )
}

fn time(days: i32, hours: i32, minutes: i32, seconds: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("TimeValue_Partitioned")
            .value("days", TestValue::Int32(days))
            .value("hours", TestValue::Int32(hours))
            .value("minutes", TestValue::Int32(minutes))
            .value("seconds", TestValue::Single(seconds)),
    )
}

/// What one synthetic blueprint holds.
pub(crate) struct BlueprintSpec {
    pub(crate) guid: u128,
    pub(crate) class: &'static str,
    pub(crate) path: &'static str,
    pub(crate) produces: u128,
    pub(crate) mandatory: TestValue,
    pub(crate) optional: Vec<TestValue>,
}

fn blueprint(spec: &BlueprintSpec) -> TestRecord {
    let costs = TestInstance::new("CraftingRecipeCosts")
        .value("craftTime", time(1, 2, 3, 3.6))
        .value("mandatoryCost", spec.mandatory.clone())
        .value("optionalCosts", TestValue::Array(spec.optional.clone()));
    let tier = TestInstance::new("CraftingBlueprintTier").value(
        "recipe",
        TestValue::Struct(
            TestInstance::new("CraftingRecipe").value("costs", TestValue::Struct(costs)),
        ),
    );
    TestRecord::new(
        spec.guid,
        "CraftingBlueprintRecord",
        format!("CraftingBlueprintRecord.{}", spec.class),
        format!("libs/foundry/records/crafting/blueprints/{}.xml", spec.path),
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new("CraftingBlueprint")
                .value("blueprintName", TestValue::text("@LOC_PLACEHOLDER"))
                .value("category", TestValue::Reference(CATEGORY))
                .value(
                    "processSpecificData",
                    TestValue::Struct(
                        TestInstance::new("CraftingProcess_Creation")
                            .value("entityClass", TestValue::Reference(spec.produces)),
                    ),
                )
                .value("tiers", TestValue::Array(vec![TestValue::Struct(tier)])),
        ),
    )
}

fn nested_blueprint() -> BlueprintSpec {
    BlueprintSpec {
        guid: NESTED,
        class: "BP_CRAFT_nested",
        path: "crafting/fpsgear/armour/bp_craft_nested",
        produces: PRODUCT,
        mandatory: select(
            "OUTER",
            "@crafting_ui_slotname_outer",
            vec![select(
                "",
                "@LOC_PLACEHOLDER",
                vec![select("", "", vec![resource(RESOURCE_GOLD, 3.0, 200)])],
            )],
        ),
        optional: vec![
            resource(RESOURCE_IRON, 0.75, 400),
            select("EXTRA", "@LOC_PLACEHOLDER", vec![item(ORE, 2, 0)]),
        ],
    }
}

fn extra_blueprints() -> Vec<BlueprintSpec> {
    let template = BlueprintSpec {
        guid: TEMPLATE,
        class: "BP_CRAFT_weapon",
        path: "crafting/fpsgear/weapons/$templates/bp_craft_weapon",
        produces: PRODUCT,
        mandatory: resource(RESOURCE_IRON, 1.0, 0),
        optional: Vec::new(),
    };
    let leftover = BlueprintSpec {
        guid: 0x1500_0000_0000_4000_8000_0000_0000_0005,
        class: "BP_CRAFT_dev_test",
        path: "crafting/fpsgear/bp_craft_dev_test",
        produces: PRODUCT,
        mandatory: resource(RESOURCE_IRON, 1.0, 0),
        optional: Vec::new(),
    };
    vec![template, leftover, nested_blueprint()]
}

/// The blueprints of the fixture in the order of their GUIDs.
pub(crate) fn blueprints() -> Vec<BlueprintSpec> {
    let magazine = BlueprintSpec {
        guid: MAGAZINE,
        class: "BP_CRAFT_behr_lmg_01_mag",
        path: "crafting/fpsgear/ammo/bp_craft_behr_lmg_01_mag",
        produces: PRODUCT,
        mandatory: select(
            "ASPECTS",
            "@LOC_PLACEHOLDER",
            vec![
                select(
                    "MAGAZINE",
                    "@crafting_ui_slotname_magazine",
                    vec![resource(RESOURCE_GOLD, 0.5, 0)],
                ),
                select(
                    "CORE",
                    "@LOC_PLACEHOLDER",
                    vec![item(ORE, 5, 500), resource(RESOURCE_IRON, 0.25, 300)],
                ),
                resource(RESOURCE_IRON, 2.0, 100),
            ],
        ),
        optional: Vec::new(),
    };
    let core = BlueprintSpec {
        guid: CORE,
        class: "BP_CRAFT_core",
        path: "crafting/vehiclegear/powerplant/bp_craft_core",
        produces: ORE,
        mandatory: resource(RESOURCE_GOLD, 1.0, 0),
        optional: Vec::new(),
    };
    let dangling = BlueprintSpec {
        guid: DANGLING,
        class: "BP_CRAFT_dangling",
        path: "crafting/vehiclegear/cooler/bp_craft_dangling",
        produces: MISSING,
        mandatory: item(ORE, 1, 0),
        optional: Vec::new(),
    };
    let mut specs = vec![magazine, core, dangling];
    specs.extend(extra_blueprints());
    specs
}

/// Returns the DataForge of the fixture.
pub(crate) fn database() -> DataForgeBuilder {
    let builder = commodity_records(structures::structures(DataForgeBuilder::new().version(8)));
    blueprints()
        .iter()
        .fold(blueprint_records(builder), |builder, spec| {
            builder.record(blueprint(spec))
        })
}

fn category(guid: u128, class: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "BlueprintCategoryRecord",
        format!("BlueprintCategoryRecord.{class}"),
        "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase.xml",
    )
}

fn blueprint_records(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .record(category(CATEGORY, "Ammo"))
        .record(category(CATEGORY_ARMOUR, "FPSArmours"))
        .record(category(CATEGORY_LEFTOVER, "Test_Category"))
        .record(
            TestRecord::new(
                CATEGORY_DATABASE,
                "BlueprintCategoryDatabaseRecord",
                "BlueprintCategoryDatabaseRecord.BlueprintCategoryDatabase",
                "libs/foundry/records/crafting/blueprintcategories/blueprintcategorydatabase.xml",
            )
            .value(
                "categories",
                TestValue::Array(vec![
                    TestValue::Reference(CATEGORY),
                    TestValue::Reference(CATEGORY_ARMOUR),
                ]),
            ),
        )
        .record(prop(PRODUCT, "Magazine"))
        .record(prop(ORE, "Ore"))
        .record(
            TestRecord::new(
                0x1600_0000_0000_4000_8000_0000_0000_0006,
                "CraftingBlueprintRecord",
                "CraftingBlueprintRecord.GlobalGenericDismantle",
                "libs/foundry/records/crafting/blueprints/dismantle/globalgenericdismantle.xml",
            )
            .value(
                "blueprint",
                TestValue::Struct(
                    TestInstance::new("GenericCraftingBlueprint").value(
                        "processSpecificData",
                        TestValue::Struct(
                            TestInstance::new("CraftingProcess_Creation")
                                .value("entityClass", TestValue::Reference(PRODUCT)),
                        ),
                    ),
                ),
            ),
        )
}

/// Returns the English `global.ini` of the fixture.
pub(crate) fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("items_commodities_gold", "Gold")
            .text("items_commodities_gold_desc", "A precious metal.")
            .text("items_commodities_dopple", "Dopple")
            .text("items_commodities_dopple_desc", "A vice.")
            .text("items_commodities_ember", "Ember")
            .text("items_commodities_ember_desc", "A glowing mineral.")
            .text("items_commodities_type_metal", "Metal")
            .build(),
    )
}

/// Returns the German `global.ini` of the fixture.
pub(crate) fn german() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\german_(germany)\\global.ini",
        GlobalIniBuilder::new()
            .text("items_commodities_gold", "Gold")
            .text("items_commodities_gold_desc", "Ein Edelmetall.")
            .text("items_commodities_type_metal", "Metall")
            .build(),
    )
}

/// Writes the fixture Archive into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted())
        .entry(english())
        .entry(german())
        .write_to(&path)?;
    Ok(path)
}
