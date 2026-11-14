// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A synthetic Archive with manufacturers, items and ships for the P4KX catalog tests.

use std::error::Error;
use std::path::{Path, PathBuf};

use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, GlobalIniBuilder, TestEntry, TestInstance, TestRecord,
    TestStruct, TestType, TestValue,
};

/// The GUID of the manufacturer every item and ship refers to.
pub(crate) const ACAS: u128 = 0x1000_0000_0000_4000_8000_0000_0000_0001;

/// What one synthetic item holds.
pub(crate) struct ItemSpec {
    pub(crate) guid: u128,
    pub(crate) class: &'static str,
    pub(crate) path: &'static str,
    pub(crate) description: &'static str,
    pub(crate) attach: bool,
}

pub(crate) const ITEMS: [ItemSpec; 7] = [
    ItemSpec {
        guid: 0x3000_0000_0000_4000_8000_0000_0000_0001,
        class: "POWR_ACAS_S01",
        path: "scitem/ships/powerplant/powr_acas_s01",
        description: "@item_DescPOWR_ACAS_S01",
        attach: true,
    },
    ItemSpec {
        guid: 0x2000_0000_0000_4000_8000_0000_0000_0002,
        class: "BEHR_Rifle_Ballistic_01",
        path: "scitem/weapons/fps_weapons/behr_rifle_ballistic_01",
        description: "@item_DescBEHR_Rifle_Ballistic_01",
        attach: true,
    },
    ItemSpec {
        guid: 0x4000_0000_0000_4000_8000_0000_0000_0003,
        class: "Hat_01",
        path: "scitem/characters/hat_01",
        description: "@item_DescHat_01",
        attach: true,
    },
    ItemSpec {
        guid: 0x5000_0000_0000_4000_8000_0000_0000_0004,
        class: "Old_Box",
        path: "legacyitems/old_box",
        description: "@item_DescOld_Box",
        attach: true,
    },
    ItemSpec {
        guid: 0x6000_0000_0000_4000_8000_0000_0000_0005,
        class: "Counter_Shop",
        path: "scitem/usables/counter_shop",
        description: "",
        attach: false,
    },
    ItemSpec {
        guid: 0x7000_0000_0000_4000_8000_0000_0000_0006,
        class: "BEHR_Rifle_Test_01",
        path: "scitem/weapons/dev/behr_rifle_test_01",
        description: "",
        attach: true,
    },
    ItemSpec {
        guid: 0x8000_0000_0000_4000_8000_0000_0000_0007,
        class: "Crate",
        path: "props/crate",
        description: "",
        attach: true,
    },
];

const SHIP: u128 = 0x9000_0000_0000_4000_8000_0000_0000_0001;
const ROVER: u128 = 0x9300_0000_0000_4000_8000_0000_0000_0004;

/// How the items and ships of the fixture give their manufacturer.
#[derive(Clone, Copy)]
pub(crate) enum Maker {
    /// A Reference to the manufacturer Record.
    Reference,
    /// A strong pointer owning an embedded manufacturer instance.
    StrongPointer,
    /// A weak pointer to a manufacturer instance.
    WeakPointer,
}

impl Maker {
    fn field_type(self) -> TestType {
        match self {
            Self::Reference => TestType::Reference,
            Self::StrongPointer => pointer("SCItemManufacturer"),
            Self::WeakPointer => TestType::WeakPointer("SCItemManufacturer".to_owned()),
        }
    }

    fn value(self) -> TestValue {
        match self {
            Self::Reference => TestValue::Reference(ACAS),
            Self::StrongPointer => TestValue::Struct(
                TestInstance::new("SCItemManufacturer").value("Code", TestValue::text("ACAS")),
            ),
            Self::WeakPointer => TestValue::pointer(MAKER_INSTANCE),
        }
    }
}

const MAKER_INSTANCE: &str = "maker";
const SHIP_TEMPLATE: u128 = 0x9100_0000_0000_4000_8000_0000_0000_0002;
const LANDING_GEAR: u128 = 0x9200_0000_0000_4000_8000_0000_0000_0003;

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn component(name: &str) -> TestStruct {
    TestStruct::new(name).parent("DataForgeComponentParams")
}

fn structures(builder: DataForgeBuilder, maker: Maker) -> DataForgeBuilder {
    [
        TestStruct::new("SCItemLocalization")
            .field("Name", TestType::Locale)
            .field("Description", TestType::Locale),
        TestStruct::new("SCItemManufacturer")
            .field("Code", TestType::String)
            .field("Localization", class("SCItemLocalization")),
        TestStruct::new("EntityClassDefinition")
            .field("Category", TestType::String)
            .array("Components", pointer("DataForgeComponentParams")),
        TestStruct::new("DataForgeComponentParams"),
        component("SAttachableComponentParams").field("AttachDef", class("SItemDefinition")),
        TestStruct::new("SItemDefinition")
            .field("Type", TestType::Enum("EItemType".to_owned()))
            .field("SubType", TestType::Enum("EItemSubType".to_owned()))
            .field("Size", TestType::Int32)
            .field("Grade", TestType::Int32)
            .field("Tags", TestType::String)
            .field("Manufacturer", maker.field_type())
            .field("Localization", class("SCItemLocalization")),
        component("SEntityPhysicsControllerParams")
            .field("PhysType", pointer("SEntityPhysicsControllerParamsBase")),
        TestStruct::new("SEntityPhysicsControllerParamsBase"),
        TestStruct::new("SEntityRigidPhysicsControllerParams")
            .parent("SEntityPhysicsControllerParamsBase")
            .field("Mass", TestType::Single),
        component("SCItemPurchasableParams").field("displayType", TestType::Locale),
        component("VehicleComponentParams")
            .field("vehicleDefinition", TestType::String)
            .field("crewSize", TestType::Int32)
            .field("vehicleName", TestType::Locale)
            .field("vehicleDescription", TestType::Locale)
            .field("vehicleCareer", TestType::Locale)
            .field("vehicleRole", TestType::Locale)
            .field("manufacturer", maker.field_type()),
        TestStruct::new("VehicleLandingGearSystem").field("Name", TestType::String),
    ]
    .into_iter()
    .fold(builder, DataForgeBuilder::structure)
}

fn localization(name: &str, description: &str) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SCItemLocalization")
            .value("Name", TestValue::text(name))
            .value("Description", TestValue::text(description)),
    )
}

fn attach(spec: &ItemSpec, maker: Maker) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SAttachableComponentParams").value(
            "AttachDef",
            TestValue::Struct(
                TestInstance::new("SItemDefinition")
                    .value("Type", TestValue::text("PowerPlant"))
                    .value("SubType", TestValue::text("Power"))
                    .value("Size", TestValue::Int32(1))
                    .value("Grade", TestValue::Int32(3))
                    .value("Tags", TestValue::text(" flightReady  $Small "))
                    .value("Manufacturer", maker.value())
                    .value(
                        "Localization",
                        localization(&format!("@item_Name{}", spec.class), spec.description),
                    ),
            ),
        ),
    )
}

fn physics(mass: f32) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SEntityPhysicsControllerParams").value(
            "PhysType",
            TestValue::Struct(
                TestInstance::new("SEntityRigidPhysicsControllerParams")
                    .value("Mass", TestValue::Single(mass)),
            ),
        ),
    )
}

fn entity(guid: u128, class: &str, path: &str, components: Vec<TestValue>) -> TestRecord {
    TestRecord::new(
        guid,
        "EntityClassDefinition",
        format!("EntityClassDefinition.{class}"),
        format!("libs/foundry/records/entities/{path}.xml"),
    )
    .value("Category", TestValue::text("Default"))
    .value("Components", TestValue::Array(components))
}

fn item(spec: &ItemSpec, maker: Maker) -> TestRecord {
    let components = if spec.attach {
        vec![
            physics(125.5),
            attach(spec, maker),
            TestValue::Struct(TestInstance::new("SCItemPurchasableParams").value(
                "displayType",
                TestValue::text("@item_DisplayType_PowerPlant"),
            )),
        ]
    } else {
        Vec::new()
    };
    entity(spec.guid, spec.class, spec.path, components)
}

fn ship(guid: u128, class: &str, path: &str, maker: Maker) -> TestRecord {
    entity(
        guid,
        class,
        path,
        vec![TestValue::Struct(
            TestInstance::new("VehicleComponentParams")
                .value(
                    "vehicleDefinition",
                    TestValue::text("scripts/entities/vehicles/implementations/xml/acas_ship.xml"),
                )
                .value("crewSize", TestValue::Int32(2))
                .value("vehicleName", TestValue::text("@vehicle_NameACAS_Ship"))
                .value(
                    "vehicleDescription",
                    TestValue::text("@vehicle_DescACAS_Ship"),
                )
                .value(
                    "vehicleCareer",
                    TestValue::text("@vehicle_focus_transporter"),
                )
                .value("vehicleRole", TestValue::text("@LOC_PLACEHOLDER"))
                .value("manufacturer", maker.value()),
        )],
    )
}

/// Returns the DataForge of the fixture, whose items and ships give their manufacturer as `maker`.
pub(crate) fn database(maker: Maker) -> DataForgeBuilder {
    let builder = structures(
        DataForgeBuilder::new()
            .version(8)
            .enumeration("EItemType", ["PowerPlant"])
            .enumeration("EItemSubType", ["Power"]),
        maker,
    )
    .instance(
        MAKER_INSTANCE,
        TestInstance::new("SCItemManufacturer").value("Code", TestValue::text("ACAS")),
    )
    .record(
        TestRecord::new(
            ACAS,
            "SCItemManufacturer",
            "SCItemManufacturer.ACAS",
            "libs/foundry/records/scitemmanufacturer/acas.xml",
        )
        .value("Code", TestValue::text("ACAS"))
        .value(
            "Localization",
            localization("@manufacturer_NameACAS", "@manufacturer_DescACAS"),
        ),
    )
    .record(ship(SHIP, "ACAS_Ship", "spaceships/acas_ship", maker))
    .record(ship(
        ROVER,
        "ACAS_Rover",
        "groundvehicles/acas_rover",
        maker,
    ))
    .record(ship(
        SHIP_TEMPLATE,
        "ACAS_Ship_Template",
        "spaceships/acas_ship_template",
        maker,
    ))
    .record(
        TestRecord::new(
            LANDING_GEAR,
            "VehicleLandingGearSystem",
            "VehicleLandingGearSystem.ACAS_Gear",
            "libs/foundry/records/entities/scitem/ships/landingsystem/acas_gear.xml",
        )
        .value("Name", TestValue::text("gear")),
    );
    ITEMS
        .iter()
        .fold(builder, |builder, spec| builder.record(item(spec, maker)))
}

/// Returns the English `global.ini` of the fixture.
pub(crate) fn english() -> TestEntry {
    TestEntry::zstd(
        "Data\\Localization\\english\\global.ini",
        GlobalIniBuilder::new()
            .text("item_NamePOWR_ACAS_S01", "Lumen")
            .text(
                "item_DescPOWR_ACAS_S01",
                "Item Type: Power Plant\\nClass:\u{a0}Military \\n\\nSteady power.",
            )
            .text(
                "item_DescBEHR_Rifle_Ballistic_01",
                "Manufacturer: Behring\\nClass: Ballistic\\n\\nA rifle.",
            )
            .text("item_DescHat_01", "A hat for every class: none.")
            .text("item_DisplayType_PowerPlant", "Power Plant")
            .text("vehicle_NameACAS_Ship", "Ace Hauler")
            .text("vehicle_focus_transporter", "Transporter")
            .build(),
    )
}

/// Writes the fixture Archive with `texts` into `dir` and returns its path.
pub(crate) fn write_archive(dir: &Path, texts: &[TestEntry]) -> Result<PathBuf, Box<dyn Error>> {
    write_archive_with(dir, texts, Maker::Reference)
}

/// Writes the fixture Archive with `texts`, whose manufacturers are given as `maker`, into `dir`
/// and returns its path.
pub(crate) fn write_archive_with(
    dir: &Path,
    texts: &[TestEntry],
    maker: Maker,
) -> Result<PathBuf, Box<dyn Error>> {
    let path = dir.join("Data.p4k");
    texts
        .iter()
        .cloned()
        .fold(
            ArchiveBuilder::new()
                .entry(TestEntry::zstd("Data\\Game2.dcb", database(maker).build()).encrypted()),
            ArchiveBuilder::entry,
        )
        .write_to(&path)?;
    Ok(path)
}
