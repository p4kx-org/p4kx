// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    ArchiveFiles, ArchiveModels, Assembly, AssemblyOptions, ForgePalettes, Issue, IssueKind,
    LoadoutChanges, LoadoutResolver, PaletteSelection,
};
use p4kx_extraction::{Archive, DataForgeEntry, SourceLocation};
use p4kx_testkit::{TestEntry, fitted_paint_vehicle_entries, test_key};
use serde_json::Value;

use crate::assembly_support::{TestResult, fixture};

#[path = "assembly_vehicle_paint_cdf.rs"]
mod cdf;
#[path = "assembly_vehicle_paint_material.rs"]
mod material;
#[path = "assembly_vehicle_paint_patch.rs"]
mod patch;

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
fn fitted_paint_origins_distinguish_default_reference_and_accepted_replacement() -> TestResult {
    let factory = paint_evidence(&assemble("TEST_Hull_Red", &LoadoutChanges::new())?)?;
    let factory = field_selectors(&factory, "/variantKind")?;
    let default_at = "Components[0].Ports[0].defaultItem.entityClass";
    assert!(
        has_record_field(&factory, 0, default_at),
        "the physical port default selected the fitted paint"
    );
    assert!(
        has_record_field(&factory, 4, "Components[0].AttachDef.Type"),
        "the fitted item has the authored paint type"
    );
    let changes = LoadoutChanges::new().replace(PORT, "TEST_Paint_Red");
    let replacement = paint_evidence(&assemble("TEST_Hull_Red", &changes)?)?;
    let replacement = field_selectors(&replacement, "/variantKind")?;
    assert!(
        !has_record_field(&replacement, 0, default_at),
        "the overridden port default is outside the replacement decision"
    );
    assert!(
        replacement.iter().any(|source| {
            string_at(source, "/location/namespace") == Some("dataforgeRecordHeader")
                && string_at(source, "/location/indices/0") == Some("4")
                && string_at(source, "/location/field") == Some("guid")
        }),
        "the record reached by the replacement is the accepted paint"
    );
    assert!(
        has_record_field(&replacement, 4, "Components[0].AttachDef.RequiredTags"),
        "the replacement's two authored requirements enter the decision"
    );
    assert!(
        has_record_field(&replacement, 0, "Components[3].AttachDef.Tags"),
        "the hull supplies the required offer absent from the port tags"
    );
    let xml_values = replacement
        .iter()
        .filter(|source| {
            string_at(source, "/location/namespace") == Some("textXmlAttribute")
                && string_at(source, "/location/field") == Some("value")
        })
        .count();
    assert_eq!(
        xml_values, 2,
        "the applied XML offer and requirement both contributed"
    );
    Ok(())
}

fn paint_evidence(assembled: &Assembly) -> Result<Value, Box<dyn std::error::Error>> {
    let evidence = assembled
        .paint_provenance()?
        .ok_or("fitted paint origins")?;
    Ok(serde_json::to_value(evidence)?)
}

fn field_selectors<'report>(
    report: &'report Value,
    name: &str,
) -> Result<Vec<&'report Value>, Box<dyn std::error::Error>> {
    let field = report
        .get("fieldOrigins")
        .and_then(|fields| fields.get(name))
        .ok_or("paint field origin")?;
    assert_eq!(
        field.get("sourceSetComplete").and_then(Value::as_bool),
        Some(false),
        "the selected positive inputs do not establish every competing source"
    );
    let ids = field
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("paint field source-use references")?;
    let uses = report
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("observed sources")?;
    Ok(uses
        .iter()
        .filter(|usage| {
            string_at(usage, "/role") == Some("selector")
                && usage.get("id").is_some_and(|id| ids.contains(id))
        })
        .collect())
}

fn has_record_field(uses: &[&Value], record: u64, field: &str) -> bool {
    uses.iter().any(|usage| {
        usage
            .pointer("/location/recordIndex")
            .and_then(Value::as_u64)
            == Some(record)
            && string_at(usage, "/location/field") == Some(field)
    })
}

fn string_at<'value>(usage: &'value Value, path: &str) -> Option<&'value str> {
    usage.pointer(path).and_then(Value::as_str)
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
    let assembled = assemble_entries(
        "TEST_Hull_Red",
        &LoadoutChanges::new(),
        entries,
        Some(BASE_MATERIAL),
    )?;
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

#[test]
fn an_encrypted_replacement_port_contributes_to_its_painted_fields() -> TestResult {
    let entries = fitted_paint_vehicle_entries()
        .into_iter()
        .map(|entry| {
            if entry.path().ends_with("TEST_Paint.xml") {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let changes = LoadoutChanges::new().replace(PORT, "TEST_Paint_Red");
    let assembled = assemble_entries("TEST_Hull_Red", &changes, entries, Some(BASE_MATERIAL))?;
    let proof = paint_evidence(&assembled)?;
    let origins = proof.get("fieldOrigins").ok_or("paint field origins")?;
    for field in [
        "/paintGuid",
        "/appearanceTag",
        "/variantKind",
        "/geometry",
        "/material",
        "/paletteGuid",
    ] {
        let origin = origins
            .get(field)
            .ok_or_else(|| format!("missing paint field {field}"))?;
        assert_eq!(
            origin
                .get("containsEncryptedSource")
                .and_then(Value::as_bool),
            Some(true),
            "the encrypted port selection contributes to {field}"
        );
        assert_eq!(
            origin.get("sourceSetComplete").and_then(Value::as_bool),
            Some(false),
            "the recorded sources cover only the observed decision for {field}"
        );
    }
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
    assemble_entries(
        entity,
        changes,
        fitted_paint_vehicle_entries(),
        Some(BASE_MATERIAL),
    )
}

fn assemble_entries(
    entity: &str,
    changes: &LoadoutChanges,
    entries: Vec<TestEntry>,
    base_material: Option<&str>,
) -> Result<Assembly, Box<dyn std::error::Error>> {
    let fixture = fixture(entries)?;
    let archive = Archive::open_with_key(&fixture.path, Some(test_key()))?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge)
        .with_forge_entry(&entry)
        .with_files(&files);
    let record = resolver.entities().find(entity)?;
    drop(resolver.resolve_with(&record, changes)?);
    let tree = resolver
        .with_vehicle_appearance()
        .resolve_with(&record, changes)?;
    assert_eq!(
        tree.geometry().and_then(p4kx_assembly::Geometry::material),
        base_material,
        "the resolved Loadout keeps the source geometry"
    );
    Ok(Assembly::build_with_palettes(
        &tree,
        &ArchiveModels::new(&archive),
        &ForgePalettes::new(&forge).with_entry(&entry),
        &AssemblyOptions::default(),
    )?)
}

#[path = "assembly_vehicle_paint_factory_xml.rs"]
mod factory_xml;

fn is_model_material_name(source: &Value) -> bool {
    string_at(source, "/location/namespace") == Some("modelChunk")
        && string_at(source, "/location/field") == Some("materialLibrary")
}
