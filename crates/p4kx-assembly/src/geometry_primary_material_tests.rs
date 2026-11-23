// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{
    ArchiveBuilder, DataForgeBuilder, TestEntity, TestEntry, TestInstance, TestLoadout,
    TestLoadoutEntry, TestModel, TestMtl, TestMtlMaterial, TestPort, TestValue, entity_schema,
    tint_palette_record,
};

use crate::{
    ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, IssueKind, LoadoutResolver,
    PaletteSelection, Part,
};

const MODEL: &str = "Objects/Test/cube.cgf";
const NODE: &str = "Objects/Test/selected.mtl";
const GEOMETRY: &str = "Objects/Test/geometry.mtl";
const NODE_PALETTE: u128 = 0x23;
const GEOMETRY_PALETTE: u128 = 0x24;
const MISSING_PALETTE: u128 = 0x25;
const NODE_LINK: Option<(u128, &str)> = Some((NODE_PALETTE, ""));
const GEOMETRY_LINK: Option<(u128, &str)> = Some((GEOMETRY_PALETTE, ""));

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn entity_material_drives_rendered_parts_and_keeps_the_winning_source() -> TestResult {
    let assembly = fixture_assembly()?;

    check_part(
        &assembly,
        "PrimaryMaterialRoot",
        NODE,
        ".Material.Material.path",
    )?;
    check_part(
        &assembly,
        "ConflictingMaterial",
        NODE,
        ".Material.Material.path",
    )?;
    check_part(
        &assembly,
        "EmptyMaterial",
        GEOMETRY,
        ".Geometry.Geometry.Material.path",
    )?;
    check_part(
        &assembly,
        "GeometryMaterial",
        GEOMETRY,
        ".Geometry.Geometry.Material.path",
    )?;
    Ok(())
}

#[test]
fn entity_palette_prefers_the_node_and_falls_back_with_exact_source() -> TestResult {
    let assembly = fixture_assembly()?;

    for class in ["PrimaryMaterialRoot", "ConflictingMaterial"] {
        check_palette(&assembly, class, "node", ".Material.Palette.RootRecord")?;
    }
    for class in [
        "EmptyMaterial",
        "GeometryMaterial",
        "MissingPalette",
        "NestedPalette",
    ] {
        check_palette(
            &assembly,
            class,
            "geometry",
            ".Geometry.Geometry.Palette.RootRecord",
        )?;
    }
    for class in ["MissingPalette", "NestedPalette"] {
        assert!(assembly.issues().iter().any(|issue|
            issue.name == class && matches!(issue.kind, IssueKind::Palette { .. })),
            "{class} reports its unreadable node palette");
    }
    Ok(())
}

fn fixture_assembly() -> TestResult<Assembly> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    let model = TestModel::cube();
    let library = TestMtl::single(&TestMtlMaterial::new("Fixture")).cryxml();
    ArchiveBuilder::new()
        .entry(TestEntry::stored("Data/Game2.dcb", database()))
        .entry(TestEntry::stored(format!("Data/{MODEL}"), model.cgf()))
        .entry(TestEntry::stored(
            "Data/Objects/Test/cube.cgfm",
            model.cgfm(),
        ))
        .entry(TestEntry::stored(format!("Data/{NODE}"), library.clone()))
        .entry(TestEntry::stored(format!("Data/{GEOMETRY}"), library))
        .write_to(&path)?;

    let archive = Archive::open(&path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let mut resolver = LoadoutResolver::new(&forge).with_forge_entry(&entry);
    let entity = resolver.entities().find("PrimaryMaterialRoot")?;
    let tree = resolver.resolve(&entity)?;
    let models = ArchiveModels::new(&archive);
    let palettes = ForgePalettes::new(&forge).with_entry(&entry);
    Ok(Assembly::build_with_palettes(
        &tree,
        &models,
        &palettes,
        &AssemblyOptions::default(),
    )?)
}

fn check_part(assembly: &Assembly, class: &str, library: &str, field: &str) -> TestResult {
    let part = assembly
        .parts()
        .iter()
        .find(|part| part.class_name() == class)
        .ok_or("the named Entity is in the rendered Assembly")?;
    assert!(
        part.materials().library().is_some(),
        "{class} has a readable material library, got {:?} with {:?}",
        part.materials(),
        part.material_selector()
    );
    assert_eq!(
        part.materials().target(),
        Some(format!("Data/{library}").as_str()),
        "{class} uses the selected physical library"
    );
    assert_eq!(
        part.material_selector(),
        Some(component_field(part, field)?)
    );
    Ok(())
}

fn check_palette(assembly: &Assembly, class: &str, name: &str, field: &str) -> TestResult {
    let part = assembly
        .parts()
        .iter()
        .find(|part| part.class_name() == class)
        .ok_or("the named Entity is in the rendered Assembly")?;
    assert_eq!(
        part.palette().map(|palette| palette.name.as_str()),
        Some(name),
        "{class}"
    );
    let source = part
        .palette_source()
        .ok_or("the rendered palette has a source")?;
    let PaletteSelection::Named { location } = source.selection() else {
        return Err("the palette is selected by an original Record field".into());
    };
    assert_eq!(
        location.as_ref(),
        Some(&component_field(part, field)?),
        "{class}"
    );
    assert!(source.records().palette.is_some());
    assert!(source.named_origin().is_some());
    assert!(source.forge_origin().is_some());
    assert_eq!(source.inherited_from(), None);
    Ok(())
}

fn component_field(part: &Part, field: &str) -> TestResult<SourceLocation> {
    let Some(SourceLocation::DataForge {
        record_index: model_record,
        field: model_field,
    }) = part.geometry_selector()
    else {
        return Err("the original model selector is available".into());
    };
    let component = model_field
        .strip_suffix(".Geometry.Geometry.Geometry.path")
        .ok_or("the original model component address")?;
    Ok(SourceLocation::DataForge {
        record_index: model_record,
        field: format!("{component}{field}"),
    })
}

fn database() -> Vec<u8> {
    let root = entity(1, "PrimaryMaterialRoot")
        .component(geometry("", NODE, None, NODE_LINK))
        .port(TestPort::new("conflict"))
        .port(TestPort::new("empty"))
        .port(TestPort::new("fallback"))
        .port(TestPort::new("missing"))
        .port(TestPort::new("nested"))
        .loadout(TestLoadout::Manual(vec![
            TestLoadoutEntry::class("conflict", "ConflictingMaterial"),
            TestLoadoutEntry::class("empty", "EmptyMaterial"),
            TestLoadoutEntry::class("fallback", "GeometryMaterial"),
            TestLoadoutEntry::class("missing", "MissingPalette"),
            TestLoadoutEntry::class("nested", "NestedPalette"),
        ]));
    let conflict = entity(2, "ConflictingMaterial").component(geometry(
        GEOMETRY,
        NODE,
        GEOMETRY_LINK,
        NODE_LINK,
    ));
    let empty = entity(3, "EmptyMaterial")
        .component(geometry(GEOMETRY, "", GEOMETRY_LINK, None))
        .component(geometry(GEOMETRY, NODE, None, NODE_LINK));
    let fallback = entity(4, "GeometryMaterial")
        .geometry(MODEL, Some(GEOMETRY))
        .palette(GEOMETRY_PALETTE, "");
    let missing = entity(5, "MissingPalette").component(geometry(
        GEOMETRY,
        NODE,
        GEOMETRY_LINK,
        Some((MISSING_PALETTE, "")),
    ));
    let nested = entity(6, "NestedPalette").component(geometry(
        GEOMETRY,
        NODE,
        GEOMETRY_LINK,
        Some((NODE_PALETTE, "detail")),
    ));
    entity_schema(DataForgeBuilder::new())
        .record(root.record())
        .record(conflict.record())
        .record(empty.record())
        .record(fallback.record())
        .record(missing.record())
        .record(nested.record())
        .record(tint_palette_record(
            NODE_PALETTE,
            "node",
            [[255, 0, 0]; 3],
            [255; 3],
            [100; 3],
        ))
        .record(tint_palette_record(
            GEOMETRY_PALETTE,
            "geometry",
            [[0, 255, 0]; 3],
            [255; 3],
            [100; 3],
        ))
        .build()
}

fn entity(guid: u128, class: &str) -> TestEntity {
    TestEntity::new(guid, class, format!("entities/test/{class}.xml"))
}

fn geometry(
    base: &str,
    material: &str,
    geometry_palette: Option<(u128, &str)>,
    node_palette: Option<(u128, &str)>,
) -> TestValue {
    let resource = |type_name, path| {
        TestValue::Struct(TestInstance::new(type_name).value("path", TestValue::text(path)))
    };
    let model = palette_field(
        TestInstance::new("SGeometryDataParams")
            .value("Geometry", resource("GlobalResourceGeometry", MODEL))
            .value("Material", resource("GlobalResourceMaterial", base)),
        geometry_palette,
    );
    let node = TestInstance::new("SGeometryNodeParams").value("Geometry", TestValue::Struct(model));
    let material_node = palette_field(
        TestInstance::new("SMaterialNodeParams")
            .value("Material", resource("GlobalResourceMaterial", material)),
        node_palette,
    );
    TestValue::Struct(
        TestInstance::new("SGeometryResourceParams")
            .value("Geometry", TestValue::Struct(node))
            .value("Material", TestValue::Struct(material_node)),
    )
}

fn palette_field(instance: TestInstance, palette: Option<(u128, &str)>) -> TestInstance {
    match palette {
        Some((guid, child)) => instance.value(
            "Palette",
            TestValue::Struct(
                TestInstance::new("TintPaletteRef")
                    .value("RootRecord", TestValue::Reference(guid))
                    .value("ChildPath", TestValue::text(child)),
            ),
        ),
        None => instance,
    }
}
