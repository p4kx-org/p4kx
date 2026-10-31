// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::entity_cargo::cargo_structs;
use crate::entity_clothing::clothing_structs;
use crate::entity_vehicle::vehicle_structs;
use crate::{DataForgeBuilder, TestInstance, TestRecord, TestStruct, TestType, TestValue};

/// Adds the structs of Entities, their default Loadouts and their item ports to `builder`.
///
/// They are named and shaped as in the game data: `EntityClassDefinition` with a `Components`
/// array, `SEntityComponentDefaultLoadoutParams`, the manual and XML Loadouts, and
/// `SItemPortContainerComponentParams` with `SItemPortDef` ports.
#[must_use]
pub fn entity_schema(builder: DataForgeBuilder) -> DataForgeBuilder {
    loadout_structs()
        .into_iter()
        .chain(port_structs())
        .chain(geometry_structs())
        .chain(vehicle_structs())
        .chain(attachable_structs())
        .chain(cargo_structs())
        .chain(clothing_structs())
        .fold(builder, DataForgeBuilder::structure)
}

fn attachable_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("SAttachableComponentParams")
            .parent("DataForgeComponentParams")
            .field("AttachDef", class("SItemDefinition")),
        TestStruct::new("SItemDefinition")
            .field("Type", TestType::String)
            .field("SubType", TestType::String)
            .field("Size", TestType::Int32)
            .field("Tags", TestType::String),
    ]
}

fn geometry_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("SGeometryResourceParams")
            .parent("DataForgeComponentParams")
            .field("Geometry", class("SGeometryNodeParams"))
            .field("Material", class("SMaterialNodeParams")),
        TestStruct::new("SMaterialNodeParams")
            .field("Tags", TestType::String)
            .field("Material", class("GlobalResourceMaterial"))
            .array("materialVariants", class("SMaterialNodeParams")),
        TestStruct::new("SGeometryNodeParams")
            .field("Tags", TestType::String)
            .field("Geometry", class("SGeometryDataParams"))
            .array("SubGeometry", class("SGeometryNodeParams")),
        TestStruct::new("SGeometryDataParams")
            .field("Geometry", class("GlobalResourceGeometry"))
            .field("Material", class("GlobalResourceMaterial"))
            .field("Palette", class("TintPaletteRef")),
        TestStruct::new("GlobalResourceGeometry").field("path", TestType::String),
        TestStruct::new("GlobalResourceMaterial").field("path", TestType::String),
        TestStruct::new("TintPaletteRef")
            .field("ChildPath", TestType::String)
            .field(
                "RootRecord",
                TestType::ReferenceTo("TintPaletteTree".to_owned()),
            ),
        TestStruct::new("TintPaletteTree").field("root", class("TintPalette")),
        TestStruct::new("TintPalette")
            .field("name", TestType::String)
            .field("entryA", class("TintEntry"))
            .field("entryB", class("TintEntry"))
            .field("entryC", class("TintEntry"))
            .field("glassColor", class("SRGB8")),
        TestStruct::new("TintEntry")
            .field("glossiness", TestType::UInt8)
            .field("tintColor", class("SRGB8"))
            .field("specColor", class("SRGB8")),
        TestStruct::new("SRGB8")
            .field("r", TestType::UInt8)
            .field("g", TestType::UInt8)
            .field("b", TestType::UInt8),
    ]
}

/// Returns the Record of type `TintPaletteTree` named `name`.
///
/// The entries A to C have the sRGB bytes `tints` as tint colour and `glossiness`, the glass has
/// the sRGB bytes `glass`, and the specular colour of every entry is (59, 59, 59). Its record path is `libs/foundry/records/tintpalettes/brand/test/<name>`; the database needs
/// [`entity_schema`].
#[must_use]
pub fn tint_palette_record(
    guid: u128,
    name: &str,
    tints: [[u8; 3]; 3],
    glass: [u8; 3],
    glossiness: [u8; 3],
) -> TestRecord {
    let colour = |[r, g, b]: [u8; 3]| {
        TestValue::Struct(
            TestInstance::new("SRGB8")
                .value("r", TestValue::UInt8(r))
                .value("g", TestValue::UInt8(g))
                .value("b", TestValue::UInt8(b)),
        )
    };
    let entry = |tint: [u8; 3], glossiness: u8| {
        TestValue::Struct(
            TestInstance::new("TintEntry")
                .value("glossiness", TestValue::UInt8(glossiness))
                .value("tintColor", colour(tint))
                .value("specColor", colour([59; 3])),
        )
    };
    let root = TestInstance::new("TintPalette")
        .value("name", TestValue::text(name))
        .value("entryA", entry(tints[0], glossiness[0]))
        .value("entryB", entry(tints[1], glossiness[1]))
        .value("entryC", entry(tints[2], glossiness[2]))
        .value("glassColor", colour(glass));
    TestRecord::new(
        guid,
        "TintPaletteTree",
        format!("TintPaletteTree.{name}"),
        format!("libs/foundry/records/tintpalettes/brand/test/{name}.xml"),
    )
    .value("root", TestValue::Struct(root))
}

fn pointer(name: &str) -> TestType {
    TestType::StrongPointer(name.to_owned())
}

fn class(name: &str) -> TestType {
    TestType::Class(name.to_owned())
}

fn loadout_structs() -> Vec<TestStruct> {
    vec![
        TestStruct::new("EntityClassDefinition")
            .field("Category", TestType::String)
            .array("Components", pointer("DataForgeComponentParams")),
        TestStruct::new("DataForgeComponentParams"),
        TestStruct::new("SEntityComponentDefaultLoadoutParams")
            .parent("DataForgeComponentParams")
            .field("loadout", pointer("SItemPortLoadoutBaseParams")),
        TestStruct::new("SItemPortLoadoutBaseParams")
            .field("SkipInventoryItemsOnMissionEntities", TestType::Bool),
        TestStruct::new("SItemPortLoadoutManualParams")
            .parent("SItemPortLoadoutBaseParams")
            .array("entries", class("SItemPortLoadoutEntryParams")),
        TestStruct::new("SItemPortLoadoutXMLParams")
            .parent("SItemPortLoadoutBaseParams")
            .field("loadoutPath", TestType::String),
        TestStruct::new("SItemPortLoadoutRandomGeneratedParams")
            .parent("SItemPortLoadoutBaseParams")
            .array(
                "LoadoutAssortments",
                TestType::ReferenceTo("SLoadoutAssortment".to_owned()),
            ),
        TestStruct::new("SLoadoutAssortment")
            .field("Requirement", pointer("SLoadoutRequirementBase"))
            .array("GroupedLoadouts", class("SGroupedLoadouts")),
        TestStruct::new("SGroupedLoadouts")
            .field("Group", TestType::String)
            .field("Requirement", pointer("SLoadoutRequirementBase"))
            .array("Loadouts", pointer("SItemPortLoadoutBaseParams")),
        TestStruct::new("SLoadoutRequirementBase"),
        TestStruct::new("SLoadoutRequirementTags")
            .parent("SLoadoutRequirementBase")
            .array("RequiredTags", TestType::ReferenceTo("Tag".to_owned()))
            .array("BannedTags", TestType::ReferenceTo("Tag".to_owned())),
        TestStruct::new("SLoadoutRequirementOR")
            .parent("SLoadoutRequirementBase")
            .array("Requirements", pointer("SLoadoutRequirementBase")),
        TestStruct::new("SLoadoutRequirementNOT")
            .parent("SLoadoutRequirementBase")
            .field("RequirementToInvert", pointer("SLoadoutRequirementBase")),
        TestStruct::new("Tag"),
        TestStruct::new("SItemPortLoadoutOutfitParams")
            .parent("SItemPortLoadoutBaseParams")
            .field("BaseLoadout", pointer("SItemPortLoadoutBaseParams"))
            .array("Outfits", class("SItemPortLoadoutOutfitTagParams")),
        TestStruct::new("SItemPortLoadoutOutfitTagParams")
            .array("OutfitNameTags", TestType::String)
            .array("Loadouts", pointer("SItemPortLoadoutBaseParams")),
        TestStruct::new("SItemPortLoadoutEntryParams")
            .field("itemPortName", TestType::String)
            .field("entityClassName", TestType::String)
            .field(
                "entityClassReference",
                TestType::ReferenceTo("EntityClassDefinition".to_owned()),
            )
            .field("loadout", pointer("SItemPortLoadoutBaseParams")),
    ]
}

fn port_structs() -> Vec<TestStruct> {
    let xyz = |name: &str| {
        TestStruct::new(name)
            .field("x", TestType::Single)
            .field("y", TestType::Single)
            .field("z", TestType::Single)
    };
    vec![
        TestStruct::new("SItemPortContainerComponentParams")
            .parent("DataForgeComponentParams")
            .field("PortFlags", TestType::String)
            .array("Ports", class("SItemPortDef")),
        TestStruct::new("SItemPortDef")
            .field("Name", TestType::String)
            .field("DisplayName", TestType::Locale)
            .field("Flags", TestType::String)
            .field("MinSize", TestType::UInt32)
            .field("MaxSize", TestType::UInt32)
            .field("PortTags", TestType::String)
            .field("RequiredPortTags", TestType::String)
            .field("defaultItem", class("SItemPortDefaultItemDef"))
            .array("Types", class("SItemPortDefTypes"))
            .field(
                "AttachmentImplementation",
                pointer("SItemPortDefAttachmentImplementationBase"),
            ),
        TestStruct::new("SItemPortDefaultItemDef").field(
            "entityClass",
            TestType::ReferenceTo("EntityClassDefinition".to_owned()),
        ),
        TestStruct::new("SItemPortDefTypes")
            .field("Type", TestType::String)
            .array("SubTypes", TestType::String),
        TestStruct::new("SItemPortDefAttachmentImplementationBase"),
        TestStruct::new("SItemPortDefAttachmentImplementationBone")
            .parent("SItemPortDefAttachmentImplementationBase")
            .field("Helper", class("SItemPortDefHelperNode")),
        TestStruct::new("SItemPortDefAttachmentImplementationSkin")
            .parent("SItemPortDefAttachmentImplementationBase")
            .field("Helper", class("SItemPortDefHelperNode")),
        TestStruct::new("SItemPortDefHelperNode").field("Helper", class("SItemPortDefHelper")),
        TestStruct::new("SItemPortDefHelper")
            .field("Name", TestType::String)
            .field("ItemOffsetHelperName", TestType::String)
            .field("Offset", class("QuatT")),
        TestStruct::new("QuatT")
            .field("Rotation", class("Ang3"))
            .field("Position", class("Vec3")),
        xyz("Ang3"),
        xyz("Vec3"),
    ]
}

pub(crate) fn attachable_component(
    attach_type: &str,
    sub_type: Option<&str>,
    size: Option<i32>,
    tags: Option<&str>,
) -> TestValue {
    let mut definition =
        TestInstance::new("SItemDefinition").value("Type", TestValue::text(attach_type));
    if let Some(sub_type) = sub_type {
        definition = definition.value("SubType", TestValue::text(sub_type));
    }
    if let Some(size) = size {
        definition = definition.value("Size", TestValue::Int32(size));
    }
    if let Some(tags) = tags {
        definition = definition.value("Tags", TestValue::text(tags));
    }
    TestValue::Struct(
        TestInstance::new("SAttachableComponentParams")
            .value("AttachDef", TestValue::Struct(definition)),
    )
}
