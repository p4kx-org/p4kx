// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, Issue, IssueKind,
    LoadoutChanges, LoadoutResolver, PaletteSelection,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{TestEntry, fitted_paint_vehicle_entries};

use crate::assembly_support::{TestResult, fixture};

const PORT: &str = "hardpoint_livery";
const BASE_MODEL: &str = "Objects/Test/hull.cgf";
const BASE_MATERIAL: &str = "Objects/Test/base";

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

#[test]
fn a_missing_root_model_has_no_rendered_paint_provenance() -> TestResult {
    let omitted = [format!("Data/{BASE_MODEL}"), format!("Data/{BASE_MODEL}m")];
    let mut entries = fitted_paint_vehicle_entries();
    entries.retain(|entry| !omitted.iter().any(|path| path == entry.path()));
    let assembled = assemble_entries("TEST_Hull_Red", &LoadoutChanges::new(), entries)?;
    let root = assembled.root().ok_or("the unresolved hull")?;
    assert!(
        root.model().is_none(),
        "the fixture omits the original hull geometry"
    );
    assert!(
        assembled.paint_provenance()?.is_none(),
        "effective paint evidence belongs to a loaded hull"
    );
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
    assemble_entries(entity, changes, fitted_paint_vehicle_entries())
}

fn assemble_entries(
    entity: &str,
    changes: &LoadoutChanges,
    entries: Vec<TestEntry>,
) -> Result<Assembly, Box<dyn std::error::Error>> {
    let fixture = fixture(entries)?;
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
