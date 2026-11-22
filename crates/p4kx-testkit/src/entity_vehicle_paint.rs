// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{
    DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout, TestModel, TestMtl,
    TestMtlMaterial, TestPort, TestType, TestValue, entity_schema,
    entity_schema::attachable_component, tint_palette_record,
};

const DEFINITION: &str = "Scripts/Entities/Vehicles/Implementations/Xml/TEST_Paint.xml";
const PORT: &str = "hardpoint_livery";
const RED: u128 = 0x5611;
const BLUE: u128 = 0x5612;
const RED_PALETTE: u128 = 0x5613;
const BLUE_PALETTE: u128 = 0x5614;
const BASE_MODEL: &str = "Objects/Test/hull.cgf";
const EMBEDDED_MODEL: &str = "Objects/Test/hull_painted.cgf";
const CDF_MODEL: &str = "Objects/Test/hull_finish.cdf";
const CDF_BODY: &str = "Objects/Test/hull_finish.skin";

/// Authored material override of the selected synthetic finish CDF.
pub const FITTED_CDF_MATERIAL: &str = r#"Material="blue.mtl""#;

/// Source document of the synthetic fitted finish.
pub const FITTED_CDF_DOCUMENT: &str = concat!(
    r#"<CharacterDefinition><AttachmentList><Attachment Type="CA_SKIN" Binding="Objects/Test/hull_finish.skin" "#,
    r#"Material="blue.mtl""#,
    r#"/></AttachmentList></CharacterDefinition>"#,
);
const BASE_MATERIAL: &str = "Objects/Test/base";
const RED_MATERIAL: &str = "Objects/Test/red";
const BLUE_MATERIAL: &str = "Objects/Test/blue";
const XML_LOADOUT: &str = "Scripts/Loadouts/fixture-paint.xml";
const XML_ITEMS: &str = "Scripts/Loadouts/fixture-paint-items.xml";
const PAINT_INCLUDE: &[u8] = br#"<Loadout><Loadouts><Loadout loadout="Scripts/Loadouts/fixture-paint-items.xml"/></Loadouts></Loadout>"#;
const PAINT_ITEM: &[u8] = br#"<Loadout><Items><Item portName="hardpoint_livery" itemName="TEST_Paint_Blue"/></Items></Loadout>"#;

const IMPLEMENTATION: &str = r#"<Vehicle name="TEST_Paint">
  <Parts><Part name="hardpoint_livery" class="ItemPort">
    <ItemPort id="livery" requiredTags="Family_Blue" portTags="UnusedFixtureOffer">
      <Types><Type type="Paints"/></Types>
    </ItemPort>
  </Part></Parts>
  <Modifications><Modification name="Red"><Elems>
    <Elem idRef="livery" name="requiredTags" value="Family_Red"/>
    <Elem idRef="livery" name="portTags" value="FixturePortOffer"/>
  </Elems></Modification></Modifications>
</Vehicle>"#;

/// Returns a shared cube hull whose fitted paints select original material or subgeometry variants.
/// The two selected variants name separate palettes and material libraries.
#[must_use]
pub fn fitted_paint_vehicle_entries() -> Vec<TestEntry> {
    entries(&database(), &TestModel::cube())
}

/// Returns fitted palette variants and the shared model's competing material library.
#[must_use]
pub fn fitted_same_model_paint_vehicle_entries(model_material: &str) -> Vec<TestEntry> {
    let named = hull(0x5631, "TEST_Hull_Palette_Only_Same", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", r"objects\TEST\HULL.CGF", None)
        .variant_palette(BLUE_PALETTE, "");
    let reused = hull(0x5632, "TEST_Hull_Palette_Only_Reused", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", "", None)
        .variant_palette(BLUE_PALETTE, "");
    let material = material_hull(
        0x5633,
        "TEST_Hull_Palette_Only_Material_Node",
        vec![material_node_with_palette("Red", "", RED_PALETTE)],
    );
    let database = database()
        .record(named.record())
        .record(reused.record())
        .record(material.record());
    let cube = TestModel::cube().with_material_library(model_material);
    entries(&database, &cube)
}

/// Returns fitted finish CDFs and the competing DataForge and model material declarations.
#[must_use]
pub fn fitted_cdf_paint_vehicle_entries() -> Vec<TestEntry> {
    let inherited = hull(0x5627, "TEST_Hull_Cdf_Finish", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", CDF_MODEL, None);
    let explicit = hull(0x5628, "TEST_Hull_Cdf_Explicit", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", CDF_MODEL, Some(RED_MATERIAL));
    let same_file = hull(0x5629, "TEST_Hull_Cdf_Same", "", BLUE)
        .geometry(CDF_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", r"objects\TEST\HULL_FINISH.CDF", None);
    let plain = hull(0x5630, "TEST_Hull_Plain_Finish", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", EMBEDDED_MODEL, None);
    let database = database()
        .record(inherited.record())
        .record(explicit.record())
        .record(same_file.record())
        .record(plain.record());
    let mut output = entries(&database, &TestModel::cube());
    let body = TestModel::two_bone_skin().with_material_library("red");
    output.extend([
        TestEntry::stored(format!("Data/{CDF_MODEL}"), FITTED_CDF_DOCUMENT).encrypted(),
        TestEntry::stored(format!("Data/{CDF_BODY}"), body.cgf()),
        TestEntry::stored(format!("Data/{CDF_BODY}m"), body.cgfm()),
    ]);
    output
}

fn entries(database: &DataForgeBuilder, cube: &TestModel) -> Vec<TestEntry> {
    let painted = TestModel::cube().with_material_library("blue");
    let mut entries = vec![
        TestEntry::zstd("Data\\Game2.dcb", database.build()),
        TestEntry::stored(format!("Data/{DEFINITION}"), IMPLEMENTATION.as_bytes()),
        TestEntry::stored(format!("Data/{BASE_MODEL}"), cube.cgf()),
        TestEntry::stored(format!("Data/{BASE_MODEL}m"), cube.cgfm()),
        TestEntry::stored(format!("Data/{EMBEDDED_MODEL}"), painted.cgf()),
        TestEntry::stored(format!("Data/{EMBEDDED_MODEL}m"), painted.cgfm()),
        TestEntry::stored(format!("Data/{XML_LOADOUT}"), PAINT_INCLUDE).encrypted(),
        TestEntry::stored(format!("Data/{XML_ITEMS}"), PAINT_ITEM),
    ];
    for (path, name) in [
        (BASE_MATERIAL, "FixtureBaseHull"),
        (RED_MATERIAL, "FixtureRedHull"),
        (BLUE_MATERIAL, "FixtureBlueHull"),
    ] {
        let material = TestMtlMaterial::new(name).diffuse([0.25; 3]);
        let material = if path == BASE_MATERIAL {
            material
        } else {
            material.shader("HardSurface").layer("Primary", "1")
        };
        let library = TestMtl::single(&material);
        let library = if path == BLUE_MATERIAL {
            library.with_default_palette(
                "libs/foundry/records/tintpalettes/brand/test/synthetic_blue.xml",
            )
        } else {
            library
        };
        entries.push(TestEntry::stored(
            format!("Data/{path}.mtl"),
            library.cryxml(),
        ));
    }
    entries
}

fn database() -> DataForgeBuilder {
    let red = material_hull(
        0x5621,
        "TEST_Hull_Red",
        vec![
            material_node_with_palette("Red", RED_MATERIAL, RED_PALETTE),
            TestValue::Struct(material_node("Orphan", BASE_MATERIAL, Vec::new())),
        ],
    );
    let blue = hull(0x5622, "TEST_Hull_Blue", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", BASE_MODEL, Some(BLUE_MATERIAL))
        .variant_palette(BLUE_PALETTE, "");
    let model_less = hull(0x5623, "TEST_Hull_Model_Less", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", "", Some(BLUE_MATERIAL))
        .variant_palette(BLUE_PALETTE, "");
    let embedded = hull(0x5625, "TEST_Hull_Embedded", "", BLUE)
        .geometry(BASE_MODEL, None)
        .geometry_variant("Blue", EMBEDDED_MODEL, None);
    schema()
        .record(red.record())
        .record(blue.record())
        .record(model_less.record())
        .record(nested_hull().record())
        .record(
            paint(
                RED,
                "TEST_Paint_Red",
                "Family_Red @Red",
                "FixturePortOffer FixtureHullOffer",
            )
            .record(),
        )
        .record(paint(BLUE, "TEST_Paint_Blue", "Family_Blue @Blue", "").record())
        .record(tint_palette_record(
            RED_PALETTE,
            "synthetic_red",
            [[255, 0, 0]; 3],
            [255; 3],
            [128; 3],
        ))
        .record(tint_palette_record(
            BLUE_PALETTE,
            "synthetic_blue",
            [[0, 0, 255]; 3],
            [255; 3],
            [128; 3],
        ))
        .record(embedded.record())
        .record(factory_xml_hull().record())
}

fn factory_xml_hull() -> TestEntity {
    hull(0x5626, "TEST_Hull_XmlPaint", "", BLUE)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .geometry_variant("Blue", BASE_MODEL, Some(BLUE_MATERIAL))
        .variant_palette(BLUE_PALETTE, "")
        .loadout(TestLoadout::Xml(XML_LOADOUT.to_owned()))
}

fn schema() -> DataForgeBuilder {
    let mut builder = entity_schema(DataForgeBuilder::new());
    builder.structs = builder
        .structs
        .into_iter()
        .map(|structure| match structure.name.as_str() {
            "SMaterialNodeParams" => {
                structure.field("Palette", TestType::Class("TintPaletteRef".to_owned()))
            }
            "SItemDefinition" => structure.field("RequiredTags", TestType::String),
            _ => structure,
        })
        .collect();
    builder
}

fn nested_hull() -> TestEntity {
    material_hull(
        0x5624,
        "TEST_Hull_Nested_Paint",
        vec![TestValue::Struct(material_node(
            "Red",
            "",
            vec![
                TestValue::Struct(material_node("Red", RED_MATERIAL, Vec::new())),
                TestValue::Struct(material_node("Red", BLUE_MATERIAL, Vec::new())),
            ],
        ))],
    )
}

fn material_hull(guid: u128, name: &str, variants: Vec<TestValue>) -> TestEntity {
    let model = TestInstance::new("SGeometryNodeParams").value(
        "Geometry",
        TestValue::Struct(
            TestInstance::new("SGeometryDataParams")
                .value("Geometry", resource("GlobalResourceGeometry", BASE_MODEL))
                .value(
                    "Material",
                    resource("GlobalResourceMaterial", BASE_MATERIAL),
                ),
        ),
    );
    hull(guid, name, "Red", RED)
        .component(TestValue::Struct(
            TestInstance::new("SGeometryResourceParams")
                .value("Geometry", TestValue::Struct(model))
                .value(
                    "Material",
                    TestValue::Struct(material_node("", "", variants)),
                ),
        ))
        .component(attachable_component(
            "NOITEM_Vehicle",
            None,
            None,
            Some("FixtureHullOffer"),
        ))
}

fn material_node_with_palette(tags: &str, path: &str, palette: u128) -> TestValue {
    TestValue::Struct(
        material_node(tags, path, Vec::new()).value(
            "Palette",
            TestValue::Struct(
                TestInstance::new("TintPaletteRef")
                    .value("ChildPath", TestValue::text(""))
                    .value("RootRecord", TestValue::Reference(palette)),
            ),
        ),
    )
}

fn material_node(tags: &str, path: &str, children: Vec<TestValue>) -> TestInstance {
    TestInstance::new("SMaterialNodeParams")
        .value("Tags", TestValue::text(tags))
        .value("Material", resource("GlobalResourceMaterial", path))
        .value("materialVariants", TestValue::Array(children))
}

fn resource(kind: &str, path: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(kind).value("path", TestValue::text(path)))
}

fn hull(guid: u128, name: &str, modification: &str, paint: u128) -> TestEntity {
    TestEntity::new(guid, name, format!("entities/{name}.xml"))
        .vehicle(DEFINITION, modification)
        .port(TestPort::new(PORT).default_item(paint))
}

fn paint(guid: u128, name: &str, tags: &str, requirements: &str) -> TestEntity {
    let definition = TestInstance::new("SItemDefinition")
        .value("Type", TestValue::text("Paints"))
        .value("Tags", TestValue::text(tags))
        .value("RequiredTags", TestValue::text(requirements));
    TestEntity::new(guid, name, format!("entities/{name}.xml")).component(TestValue::Struct(
        TestInstance::new("SAttachableComponentParams")
            .value("AttachDef", TestValue::Struct(definition)),
    ))
}
