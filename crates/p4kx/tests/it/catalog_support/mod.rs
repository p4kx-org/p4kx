// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive for the `p4kx catalog` tests: manufacturers, an item, a ship, a commodity
//! and a blueprint with English and German `global.ini` files.

use std::error::Error;
use std::fs::{self, File};
use std::path::{Path, PathBuf};
use std::time::{Duration, UNIX_EPOCH};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue, write_build_manifest,
};

pub(crate) const EXPORTED_AT: &str = "1792000000";
const MODIFIED: Duration = Duration::from_hours(497_500);

fn manufacturer(guid: u128, class: &str, path: &str, code: &str) -> TestRecord {
    TestRecord::new(
        guid,
        "SCItemManufacturer",
        format!("SCItemManufacturer.{class}"),
        format!("libs/foundry/records/scitemmanufacturer/{path}.xml"),
    )
    .value("Code", TestValue::text(code))
    .value(
        "Localization",
        TestValue::Struct(
            TestInstance::new("SCItemLocalization")
                .value(
                    "Name",
                    TestValue::text(format!("@manufacturer_Name{class}")),
                )
                .value(
                    "Description",
                    TestValue::text(format!("@manufacturer_Desc{class}")),
                ),
        ),
    )
}

fn database() -> DataForgeBuilder {
    crafting(entities(manufacturers(DataForgeBuilder::new().version(8))))
}

fn manufacturers(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(
            TestStruct::new("SCItemLocalization")
                .field("Name", TestType::Locale)
                .field("Description", TestType::Locale),
        )
        .structure(
            TestStruct::new("SCItemManufacturer")
                .field("Code", TestType::String)
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        )
        .record(manufacturer(
            0xcfc0_122d_5275_415a_a656_3dcb_3346_feb5,
            "ACAS",
            "acas",
            "ACAS",
        ))
        .record(manufacturer(
            0x0a1b_0000_0000_4000_8000_0000_0000_0001,
            "AEGS",
            "scitemmanufacturer.aegs",
            "AEGS",
        ))
        .record(manufacturer(
            0x5555_0000_0000_4000_8000_0000_0000_0002,
            "Paint_Test_01",
            "paint_test_01",
            "",
        ))
}

fn entities(builder: DataForgeBuilder) -> DataForgeBuilder {
    builder
        .structure(TestStruct::new("EntityClassDefinition").array(
            "Components",
            TestType::StrongPointer("DataForgeComponentParams".to_owned()),
        ))
        .structure(TestStruct::new("DataForgeComponentParams"))
        .structure(
            TestStruct::new("SAttachableComponentParams")
                .parent("DataForgeComponentParams")
                .field("AttachDef", TestType::Class("SItemDefinition".to_owned())),
        )
        .structure(
            TestStruct::new("SItemDefinition")
                .field("Type", TestType::String)
                .field("Size", TestType::Int32)
                .field("Tags", TestType::String)
                .field("Manufacturer", TestType::Reference)
                .field(
                    "Localization",
                    TestType::Class("SCItemLocalization".to_owned()),
                ),
        )
        .structure(
            TestStruct::new("VehicleComponentParams")
                .parent("DataForgeComponentParams")
                .field("vehicleDefinition", TestType::String)
                .field("crewSize", TestType::Int32)
                .field("vehicleName", TestType::Locale)
                .field("vehicleRole", TestType::Locale)
                .field("manufacturer", TestType::Reference),
        )
        .record(entity(
            0x3333_0000_0000_4000_8000_0000_0000_0003,
            "scitem/ships/cooler/cool_acas_s01",
            cooler(),
        ))
        .record(entity(
            0x4444_0000_0000_4000_8000_0000_0000_0004,
            "spaceships/acas_hauler",
            TestInstance::new("VehicleComponentParams")
                .value(
                    "vehicleDefinition",
                    TestValue::text(
                        "scripts/entities/vehicles/implementations/xml/acas_hauler.xml",
                    ),
                )
                .value("crewSize", TestValue::Int32(2))
                .value("vehicleName", TestValue::text("@vehicle_NameACAS_Hauler"))
                .value(
                    "vehicleRole",
                    TestValue::text("@vehicle_class_lightfreight"),
                )
                .value("manufacturer", TestValue::Reference(ACAS)),
        ))
}

const ACAS: u128 = 0xcfc0_122d_5275_415a_a656_3dcb_3346_feb5;
const RESOURCE_GOLD: u128 = 0x5555_0000_0000_4000_8000_0000_0000_0005;
const GOLD: u128 = 0x6666_0000_0000_4000_8000_0000_0000_0006;
const BLUEPRINT: u128 = 0x7777_0000_0000_4000_8000_0000_0000_0007;

fn crafting(builder: DataForgeBuilder) -> DataForgeBuilder {
    let pointer = |name: &str| TestType::StrongPointer(name.to_owned());
    let class = |name: &str| TestType::Class(name.to_owned());
    let cost = |name: &str| TestStruct::new(name).parent("CraftingCost");
    [
        TestStruct::new("SCItemPurchasableParams")
            .parent("DataForgeComponentParams")
            .field("displayName", TestType::Locale)
            .field("displayType", TestType::Locale),
        TestStruct::new("ResourceType")
            .field("displayName", TestType::Locale)
            .field("description", TestType::Locale),
        TestStruct::new("CraftingBlueprintRecord")
            .field("blueprint", pointer("CraftingBlueprintBase")),
        TestStruct::new("CraftingBlueprintBase"),
        TestStruct::new("CraftingBlueprint")
            .parent("CraftingBlueprintBase")
            .field("blueprintName", TestType::Locale)
            .array("tiers", class("CraftingBlueprintTier")),
        TestStruct::new("CraftingBlueprintTier").field("recipe", class("CraftingRecipe")),
        TestStruct::new("CraftingRecipe").field("costs", class("CraftingRecipeCosts")),
        TestStruct::new("CraftingRecipeCosts")
            .field("craftTime", class("TimeValue_Partitioned"))
            .field("mandatoryCost", pointer("CraftingCost")),
        TestStruct::new("TimeValue_Partitioned")
            .field("minutes", TestType::Int32)
            .field("seconds", TestType::Single),
        TestStruct::new("CraftingCost"),
        cost("CraftingCost_Resource")
            .field("minQuality", TestType::Int32)
            .field("resource", TestType::Reference)
            .field("quantity", class("SStandardCargoUnit")),
        TestStruct::new("SStandardCargoUnit").field("standardCargoUnits", TestType::Single),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
    .record(
        TestRecord::new(
            RESOURCE_GOLD,
            "ResourceType",
            "ResourceType.Gold",
            "libs/foundry/records/resourcetypedatabase/resourcetypedatabase.xml",
        )
        .value("displayName", TestValue::text("@items_commodities_gold"))
        .value(
            "description",
            TestValue::text("@items_commodities_gold_desc"),
        ),
    )
    .record(entity_commodity())
    .record(blueprint_record())
}

fn entity_commodity() -> TestRecord {
    entity(
        GOLD,
        "commodities/metals/gold",
        TestInstance::new("SCItemPurchasableParams")
            .value("displayName", TestValue::text("@items_commodities_gold"))
            .value(
                "displayType",
                TestValue::text("@items_commodities_type_metal"),
            ),
    )
}

fn blueprint_record() -> TestRecord {
    let costs = TestInstance::new("CraftingRecipeCosts")
        .value(
            "craftTime",
            TestValue::Struct(
                TestInstance::new("TimeValue_Partitioned")
                    .value("minutes", TestValue::Int32(1))
                    .value("seconds", TestValue::Single(30.0)),
            ),
        )
        .value(
            "mandatoryCost",
            TestValue::Struct(
                TestInstance::new("CraftingCost_Resource")
                    .value("minQuality", TestValue::Int32(200))
                    .value("resource", TestValue::Reference(RESOURCE_GOLD))
                    .value(
                        "quantity",
                        TestValue::Struct(
                            TestInstance::new("SStandardCargoUnit")
                                .value("standardCargoUnits", TestValue::Single(0.5)),
                        ),
                    ),
            ),
        );
    let tier = TestInstance::new("CraftingBlueprintTier").value(
        "recipe",
        TestValue::Struct(
            TestInstance::new("CraftingRecipe").value("costs", TestValue::Struct(costs)),
        ),
    );
    TestRecord::new(
        BLUEPRINT,
        "CraftingBlueprintRecord",
        "CraftingBlueprintRecord.BP_CRAFT_gold_plate",
        "libs/foundry/records/crafting/blueprints/crafting/vehiclegear/bp_craft_gold_plate.xml",
    )
    .value(
        "blueprint",
        TestValue::Struct(
            TestInstance::new("CraftingBlueprint")
                .value("blueprintName", TestValue::text("@crafting_gold_plate"))
                .value("tiers", TestValue::Array(vec![TestValue::Struct(tier)])),
        ),
    )
}

fn cooler() -> TestInstance {
    TestInstance::new("SAttachableComponentParams").value(
        "AttachDef",
        TestValue::Struct(
            TestInstance::new("SItemDefinition")
                .value("Type", TestValue::text("Cooler"))
                .value("Size", TestValue::Int32(1))
                .value("Tags", TestValue::text("flightReady"))
                .value("Manufacturer", TestValue::Reference(ACAS))
                .value(
                    "Localization",
                    TestValue::Struct(
                        TestInstance::new("SCItemLocalization")
                            .value("Name", TestValue::text("@item_NameCOOL_ACAS_S01"))
                            .value("Description", TestValue::text("@item_DescCOOL_ACAS_S01")),
                    ),
                ),
        ),
    )
}

fn entity(guid: u128, path: &str, component: TestInstance) -> TestRecord {
    let class = path.rsplit('/').next().unwrap_or(path);
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/{path}.xml"),
    )
    .value(
        "Components",
        TestValue::Array(vec![TestValue::Struct(component)]),
    )
}

pub(crate) fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("manufacturer_NameACAS", "Ace Astrogation")
            .text("manufacturer_DescACAS", "Power plants.")
            .text("manufacturer_NameAEGS,P", "Aegis Dynamics")
            .text("item_NameCOOL_ACAS_S01", "Blizzard")
            .text(
                "item_DescCOOL_ACAS_S01",
                "Item Type: Cooler\\nClass: Industrial\\n\\nKeeps cool.",
            )
            .text("vehicle_NameACAS_Hauler", "Ace Hauler")
            .text("vehicle_class_lightfreight", "Light Freight")
            .text("items_commodities_gold", "Gold")
            .text("items_commodities_gold_desc", "A precious metal.")
            .text("items_commodities_type_metal", "Metal")
            .text("crafting_gold_plate", "Gold Plate")
            .build(),
    )
}

pub(crate) fn german() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\german_(germany)\\global.ini",
        GlobalIniBuilder::new()
            .text("manufacturer_NameACAS", "Ace Astrogation")
            .text("manufacturer_DescACAS", "Kraftwerke.")
            .build(),
    )
}

pub(crate) fn write_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with(dir, &[english(), german()])
}

pub(crate) fn write_archive_with(
    dir: &Path,
    texts: &[TestEntry],
) -> Result<PathBuf, Box<dyn Error>> {
    let forge = TestEntry::zstd("Data\\Game2.dcb", database().build()).encrypted();
    write_entries(dir, std::iter::once(forge).chain(texts.iter().cloned()))
}

/// Writes an Archive whose DataForge is plain and whose vehicle file of the ship is encrypted, so
/// that the catalog needs the Archive key for the file only.
pub(crate) fn write_encrypted_vehicle_archive(dir: &Path) -> Result<PathBuf, Box<dyn Error>> {
    write_entries(
        dir,
        [
            TestEntry::zstd("Data\\Game2.dcb", database().build()),
            english(),
            TestEntry::zstd(
                "Data\\Scripts\\Entities\\Vehicles\\Implementations\\Xml\\ACAS_Hauler.xml",
                b"<Vehicle name=\"ACAS_Hauler\"/>".to_vec(),
            )
            .encrypted(),
        ],
    )
}

fn write_entries(
    dir: &Path,
    entries: impl IntoIterator<Item = TestEntry>,
) -> Result<PathBuf, Box<dyn Error>> {
    let channel = dir.join("LIVE");
    fs::create_dir(&channel)?;
    write_build_manifest(
        &channel,
        "sc-alpha-4.4.0",
        "4.4.162.12345",
        "10012345",
        "0123456789abcdef",
    )?;
    let path = channel.join("Data.p4k");
    entries
        .into_iter()
        .fold(ArchiveBuilder::new(), ArchiveBuilder::entry)
        .write_to(&path)?;
    File::options()
        .write(true)
        .open(&path)?
        .set_modified(UNIX_EPOCH + MODIFIED)?;
    Ok(path)
}
