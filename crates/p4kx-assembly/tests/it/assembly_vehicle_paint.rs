// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, Issue, IssueKind,
    LoadoutChanges, LoadoutResolver, PaletteSelection,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{
    DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestModel, TestMtl, TestMtlMaterial,
    TestPort, TestValue, entity_schema, tint_palette_record,
};

use crate::assembly_support::{TestResult, fixture};

const DEFINITION: &str = "Scripts/Entities/Vehicles/Implementations/Xml/TEST_Paint.xml";
const PORT: &str = "hardpoint_livery";
const RED: u128 = 0x5611;
const BLUE: u128 = 0x5612;
const PALETTE: u128 = 0x5613;
const BASE_MODEL: &str = "Objects/Test/hull.cgf";
const BASE_MATERIAL: &str = "Objects/Test/base";
const RED_MATERIAL: &str = "Objects/Test/red";
const BLUE_MATERIAL: &str = "Objects/Test/blue";

const IMPLEMENTATION: &str = r#"<Vehicle name="TEST_Paint">
  <Parts><Part name="hardpoint_livery" class="ItemPort">
    <ItemPort id="livery" requiredTags="Family_Blue">
      <Types><Type type="Paints"/></Types>
    </ItemPort>
  </Part></Parts>
  <Modifications><Modification name="Red"><Elems>
    <Elem idRef="livery" name="requiredTags" value="Family_Red"/>
  </Elems></Modification></Modifications>
</Vehicle>"#;

#[test]
fn equipped_paint_selects_the_material_tree_and_applied_port_requirement() -> TestResult {
    let with_factory = LoadoutChanges::new();
    let red = LoadoutChanges::new().replace(PORT, "TEST_Paint_Red");
    let blue = LoadoutChanges::new().replace(PORT, "TEST_Paint_Blue");
    for changes in [&with_factory, &red] {
        let assembled = assemble("TEST_Hull_Red", changes)?;
        let root = assembled.root().ok_or("the hull")?;
        let field = field_of(root.material_selector()).ok_or("material source")?;
        assert!(
            field.ends_with(".Material.materialVariants[0].Material.path"),
            "{field}"
        );
        assert!(root.materials().library().is_some());
        assert!(!assembled.issues().iter().any(is_paint_issue));
    }
    let incompatible = assemble("TEST_Hull_Red", &blue)?;
    let root = incompatible.root().ok_or("the hull")?;
    let field = field_of(root.material_selector()).ok_or("material source")?;
    assert!(
        field.ends_with(".Geometry.Geometry.Material.path"),
        "{field}"
    );
    assert!(incompatible.issues().iter().any(|issue| {
        matches!(
            issue.kind(),
            IssueKind::PaintAppearance { port, reason }
                if port == PORT && reason.contains("does not satisfy")
        )
    }));
    Ok(())
}

#[test]
fn equipped_paint_reads_the_tagged_subgeometry_and_its_authored_palette() -> TestResult {
    let assembled = assemble("TEST_Hull_Blue", &LoadoutChanges::new())?;
    let root = assembled.root().ok_or("the hull")?;
    let material = field_of(root.material_selector()).ok_or("the material choice")?;
    assert!(
        material.ends_with(".Geometry.SubGeometry[0].Geometry.Material.path"),
        "{material}"
    );
    let palette = root.palette().ok_or("the fitted palette")?;
    assert_eq!(palette.name, "synthetic_blue");
    let source = root.palette_source().ok_or("the palette provenance")?;
    let PaletteSelection::Named { location } = source.selection() else {
        return Err("the source is the selected subgeometry".into());
    };
    let field = field_of(location.clone()).ok_or("the original palette field")?;
    assert!(
        field.ends_with(".Geometry.SubGeometry[0].Geometry.Palette.RootRecord"),
        "{field}"
    );
    assert!(!assembled.issues().iter().any(is_paint_issue));
    Ok(())
}

#[test]
fn a_model_less_paint_variant_uses_the_base_model_with_its_own_material_and_palette() -> TestResult
{
    let assembled = assemble("TEST_Hull_Model_Less", &LoadoutChanges::new())?;
    let root = assembled.root().ok_or("the hull")?;
    assert_eq!(root.geometry(), Some(BASE_MODEL));
    let model = field_of(root.geometry_selector()).ok_or("the base model source")?;
    assert!(
        model.ends_with(".Geometry.Geometry.Geometry.path"),
        "{model}"
    );
    let material = field_of(root.material_selector()).ok_or("the painted material source")?;
    assert!(
        material.ends_with(".Geometry.SubGeometry[0].Geometry.Material.path"),
        "{material}"
    );
    assert_eq!(
        root.palette().ok_or("the painted palette")?.name,
        "synthetic_blue"
    );
    let source = root.palette_source().ok_or("the painted palette source")?;
    let PaletteSelection::Named { location } = source.selection() else {
        return Err("the source is the selected subgeometry".into());
    };
    let field = field_of(location.clone()).ok_or("the original palette field")?;
    assert!(
        field.ends_with(".Geometry.SubGeometry[0].Geometry.Palette.RootRecord"),
        "{field}"
    );
    assert!(!assembled.issues().iter().any(is_paint_issue));
    Ok(())
}

#[test]
fn competing_nested_material_siblings_keep_the_base_appearance() -> TestResult {
    let assembled = assemble("TEST_Hull_Nested_Paint", &LoadoutChanges::new())?;
    let root = assembled.root().ok_or("the hull")?;
    let material = field_of(root.material_selector()).ok_or("the base material source")?;
    assert!(
        material.ends_with(".Geometry.Geometry.Material.path"),
        "{material}"
    );
    assert!(assembled.issues().iter().any(|issue| {
        matches!(
            issue.kind(),
            IssueKind::PaintAppearance { port, reason }
                if port == PORT && *reason == "the appearance tag selects more than one source variant"
        )
    }));
    Ok(())
}

const fn is_paint_issue(issue: &Issue) -> bool {
    matches!(issue.kind(), IssueKind::PaintAppearance { .. })
}

fn field_of(source: Option<SourceLocation>) -> Option<String> {
    match source? {
        SourceLocation::DataForge { field, .. } => Some(field),
        _ => None,
    }
}

fn assemble(
    entity: &str,
    changes: &LoadoutChanges,
) -> Result<Assembly, Box<dyn std::error::Error>> {
    let fixture = fixture(entries())?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_forge_entry(&entry)
        .with_files(&files)
        .with_vehicle_appearance();
    let record = resolver.entities().find(entity)?;
    let tree = resolver.resolve_with(&record, changes)?;
    assert_eq!(
        tree.geometry().and_then(p4kx_assembly::Geometry::material),
        Some(BASE_MATERIAL),
        "the resolved Loadout keeps the source geometry"
    );
    Ok(Assembly::build_with_palettes(
        &tree,
        &ArchiveModels::new(&archive),
        &ForgePalettes::new(&forge).with_entry(&entry),
        &AssemblyOptions::default(),
    )?)
}

fn entries() -> Vec<TestEntry> {
    let red = hull(0x5621, "TEST_Hull_Red", "Red", RED)
        .material_choice("Red", RED_MATERIAL)
        .material_choice("Orphan", BASE_MATERIAL);
    let blue = hull(0x5622, "TEST_Hull_Blue", "", BLUE)
        .geometry_variant("Blue", BASE_MODEL, Some(BLUE_MATERIAL))
        .variant_palette(PALETTE, "");
    let model_less = hull(0x5623, "TEST_Hull_Model_Less", "", BLUE)
        .geometry_variant("Blue", "", Some(BLUE_MATERIAL))
        .variant_palette(PALETTE, "");
    let forge = entity_schema(DataForgeBuilder::new())
        .record(red.record())
        .record(blue.record())
        .record(model_less.record())
        .record(nested_hull().record())
        .record(paint(RED, "TEST_Paint_Red", "Family_Red @Red").record())
        .record(paint(BLUE, "TEST_Paint_Blue", "Family_Blue @Blue").record())
        .record(tint_palette_record(
            PALETTE,
            "synthetic_blue",
            [[64, 128, 255]; 3],
            [255; 3],
            [128; 3],
        ))
        .build();
    let cube = TestModel::cube();
    let mut entries = vec![
        TestEntry::zstd("Data\\Game2.dcb", forge),
        TestEntry::stored(format!("Data/{DEFINITION}"), IMPLEMENTATION.as_bytes()),
        TestEntry::stored(format!("Data/{BASE_MODEL}"), cube.cgf()),
        TestEntry::stored(format!("Data/{BASE_MODEL}m"), cube.cgfm()),
    ];
    let library = TestMtl::single(&TestMtlMaterial::new("Hull")).cryxml();
    for path in [BASE_MATERIAL, RED_MATERIAL, BLUE_MATERIAL] {
        entries.push(TestEntry::stored(format!("Data/{path}.mtl"), &library[..]));
    }
    entries
}

fn nested_hull() -> TestEntity {
    let materials = material_node(
        "",
        "",
        vec![material_node(
            "Red",
            "",
            vec![
                material_node("Red", RED_MATERIAL, Vec::new()),
                material_node("Red", BLUE_MATERIAL, Vec::new()),
            ],
        )],
    );
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
    TestEntity::new(
        0x5624,
        "TEST_Hull_Nested_Paint",
        "entities/test_hull_nested_paint.xml",
    )
    .vehicle(DEFINITION, "Red")
    .port(TestPort::new(PORT).default_item(RED))
    .component(TestValue::Struct(
        TestInstance::new("SGeometryResourceParams")
            .value("Geometry", TestValue::Struct(model))
            .value("Material", materials),
    ))
}

fn material_node(tags: &str, path: &str, children: Vec<TestValue>) -> TestValue {
    TestValue::Struct(
        TestInstance::new("SMaterialNodeParams")
            .value("Tags", TestValue::text(tags))
            .value("Material", resource("GlobalResourceMaterial", path))
            .value("materialVariants", TestValue::Array(children)),
    )
}

fn resource(kind: &str, path: &str) -> TestValue {
    TestValue::Struct(TestInstance::new(kind).value("path", TestValue::text(path)))
}

fn hull(guid: u128, name: &str, modification: &str, paint: u128) -> TestEntity {
    TestEntity::new(guid, name, format!("entities/{name}.xml"))
        .vehicle(DEFINITION, modification)
        .geometry(BASE_MODEL, Some(BASE_MATERIAL))
        .port(TestPort::new(PORT).default_item(paint))
}

fn paint(guid: u128, name: &str, tags: &str) -> TestEntity {
    TestEntity::new(guid, name, format!("entities/{name}.xml"))
        .attach_type("Paints")
        .tags(tags)
}
