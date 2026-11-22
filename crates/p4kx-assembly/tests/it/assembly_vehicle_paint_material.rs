// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{IssueKind, LoadoutChanges};
use p4kx_model::MaterialStatus;
use p4kx_testkit::{
    FITTED_CDF_DOCUMENT, FITTED_CDF_MATERIAL, TestEntry, fitted_cdf_paint_vehicle_entries,
    fitted_paint_vehicle_entries, fitted_same_model_paint_vehicle_entries,
};
use serde_json::{Value, json};

use super::{
    BASE_MATERIAL, BASE_MODEL, assemble, assemble_entries, field_of, field_selectors,
    paint_evidence, string_at,
};
use crate::assembly_support::TestResult;

#[test]
fn embedded_paint_material_and_default_palette_use_the_selected_model_chunk() -> TestResult {
    let entries = fitted_paint_vehicle_entries()
        .into_iter()
        .map(|entry| {
            if entry.path().ends_with("hull_painted.cgf") {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect();
    let assembled = assemble_entries("TEST_Hull_Embedded", &LoadoutChanges::new(), entries, None)?;
    let report = paint_evidence(&assembled)?;
    assert_eq!(
        string_at(&report, "/appearance/material"),
        Some("Data/Objects/Test/blue.mtl"),
        "the painted subgeometry resolves the bare material name beside its model"
    );
    for field in ["material", "paletteGuid"] {
        assert_encrypted_model_source(&report, field)?;
    }
    assert!(
        field_selectors(&report, "/paletteGuid")?
            .iter()
            .any(|source| {
                string_at(source, "/location/field") == Some("DefaultPalette")
                    && string_at(source, "/effectiveEncryption") == Some("unencrypted")
            }),
        "the public MTL declares the selected default palette"
    );
    let overridden = assemble("TEST_Hull_Blue", &LoadoutChanges::new())?;
    let report = paint_evidence(&overridden)?;
    assert!(
        !field_selectors(&report, "/material")?
            .into_iter()
            .any(is_model_material_name),
        "an explicit subgeometry material suppresses the model's embedded name"
    );
    Ok(())
}

#[test]
fn fitted_palette_only_subgeometry_uses_the_shared_models_material() -> TestResult {
    for vehicle in [
        "TEST_Hull_Palette_Only_Reused",
        "TEST_Hull_Palette_Only_Same",
    ] {
        let entries = fitted_same_model_paint_vehicle_entries("red")
            .into_iter()
            .map(|entry| {
                if entry.path().ends_with(BASE_MODEL) {
                    entry.encrypted()
                } else {
                    entry
                }
            })
            .collect();
        let assembly = assemble_entries(
            vehicle,
            &LoadoutChanges::new(),
            entries,
            Some(BASE_MATERIAL),
        )?;
        let root = assembly.root().ok_or("the fitted hull")?;
        let report = paint_evidence(&assembly)?;
        assert_eq!(
            string_at(&report, "/appearance/variantKind"),
            Some("subGeometry"),
            "{vehicle} selects a physical geometry node"
        );
        assert_eq!(
            root.material_archive_origins()
                .library()
                .map(|entry| entry.entry().path()),
            Some("Data/Objects/Test/red.mtl"),
            "{vehicle} draws with the shared model's embedded material"
        );
        assert_eq!(
            string_at(&report, "/appearance/material"),
            Some("Data/Objects/Test/red.mtl"),
            "{vehicle} report names the rendered material"
        );
        assert!(
            root.material_selector().is_none(),
            "{vehicle} material is free of a DataForge material selector"
        );
        assert_eq!(
            root.palette().ok_or("the fitted palette")?.name,
            "synthetic_blue"
        );
        assert_encrypted_model_source(&report, "material")?;
    }
    Ok(())
}

#[test]
fn fitted_palette_only_subgeometry_inherits_material_when_the_model_names_none() -> TestResult {
    let assembly = assemble_entries(
        "TEST_Hull_Palette_Only_Reused",
        &LoadoutChanges::new(),
        fitted_same_model_paint_vehicle_entries(""),
        Some(BASE_MATERIAL),
    )?;
    let root = assembly.root().ok_or("the fitted hull")?;
    assert!(
        root.model()
            .ok_or("the shared model")?
            .material_library
            .is_none()
    );
    let report = paint_evidence(&assembly)?;
    assert_eq!(
        string_at(&report, "/appearance/material"),
        Some("Data/Objects/Test/base.mtl")
    );
    assert_eq!(
        root.palette().ok_or("the fitted palette")?.name,
        "synthetic_blue"
    );
    let source = field_of(root.material_selector()).ok_or("the base material selector")?;
    assert!(
        source.ends_with(".Geometry.Geometry.Material.path"),
        "{source}"
    );
    assert!(
        field_selectors(&report, "/material")?
            .into_iter()
            .all(|usage| !is_model_material_name(usage))
    );
    Ok(())
}

#[test]
fn named_model_material_failures_keep_the_selected_material_diagnostic() -> TestResult {
    for model_material in ["missing_fitted_material", "../invalid_fitted_material"] {
        let assembly = assemble_entries(
            "TEST_Hull_Palette_Only_Reused",
            &LoadoutChanges::new(),
            fitted_same_model_paint_vehicle_entries(model_material),
            Some(BASE_MATERIAL),
        )?;
        let root = assembly.root().ok_or("the fitted hull")?;
        let material = root.materials();
        let path = match material {
            MaterialStatus::Missing { path, .. } if model_material == "missing_fitted_material" => {
                path
            }
            MaterialStatus::Unreadable {
                path, target: None, ..
            } if model_material == "../invalid_fitted_material" => path,
            _ => return Err(format!("{model_material}: unexpected material {material:?}").into()),
        };
        assert_eq!(path, model_material);
        assert!(root.material_selector().is_none());
        assert!(
            assembly.issues().iter().any(|issue| matches!(
                issue.kind(),
                IssueKind::MaterialLibrary { reason } if reason.contains(model_material)
            )),
            "{model_material} should be reported"
        );
    }
    Ok(())
}

#[test]
fn explicit_subgeometry_and_palette_only_material_variants_keep_their_materials() -> TestResult {
    for (vehicle, kind, library, field, palette) in [
        (
            "TEST_Hull_Model_Less",
            "subGeometry",
            "Data/Objects/Test/blue.mtl",
            ".Geometry.SubGeometry[0].Geometry.Material.path",
            "synthetic_blue",
        ),
        (
            "TEST_Hull_Palette_Only_Material_Node",
            "materialVariant",
            "Data/Objects/Test/base.mtl",
            ".Geometry.Geometry.Material.path",
            "synthetic_red",
        ),
    ] {
        let assembly = assemble_entries(
            vehicle,
            &LoadoutChanges::new(),
            fitted_same_model_paint_vehicle_entries("red"),
            Some(BASE_MATERIAL),
        )?;
        let root = assembly.root().ok_or("the fitted hull")?;
        let report = paint_evidence(&assembly)?;
        assert_eq!(
            string_at(&report, "/appearance/variantKind"),
            Some(kind),
            "{vehicle}"
        );
        assert_eq!(
            string_at(&report, "/appearance/material"),
            Some(library),
            "{vehicle}"
        );
        assert_eq!(
            root.palette().ok_or("the fitted palette")?.name,
            palette,
            "{vehicle}"
        );
        let source = field_of(root.material_selector()).ok_or("the authored material selector")?;
        assert!(source.ends_with(field), "{vehicle} uses {source}");
        assert!(
            field_selectors(&report, "/material")?
                .into_iter()
                .all(|usage| !is_model_material_name(usage)),
            "{vehicle} material does not depend on the embedded name"
        );
    }
    Ok(())
}

#[test]
fn a_distinct_finish_cdf_supplies_its_authored_material_and_exact_origins() -> TestResult {
    let assembly = assemble_entries(
        "TEST_Hull_Cdf_Finish",
        &LoadoutChanges::new(),
        fitted_cdf_paint_vehicle_entries(),
        Some(BASE_MATERIAL),
    )?;
    let root = assembly.root().ok_or("the painted part")?;
    assert!(
        root.material_selector().is_none(),
        "the inherited material did not win"
    );
    assert_eq!(
        root.material_archive_origins()
            .library()
            .map(|entry| entry.entry().path()),
        Some("Data/Objects/Test/blue.mtl"),
        "the renderer reads the CDF's chosen material library"
    );
    let report = paint_evidence(&assembly)?;
    assert_eq!(
        string_at(&report, "/appearance/material"),
        Some("Data/Objects/Test/blue.mtl"),
        "the selected CDF outranks the inherited DataForge material"
    );
    let start = FITTED_CDF_DOCUMENT
        .find(FITTED_CDF_MATERIAL)
        .ok_or("the source attribute")?;
    let end = start + FITTED_CDF_MATERIAL.len();
    let original = json!([start.to_string(), end.to_string()]);
    for field in ["/material", "/paletteGuid"] {
        let sources = field_selectors(&report, field)?;
        assert!(
            sources.iter().any(|source| {
                is_cdf_material(source)
                    && source.pointer("/location/indices") == Some(&original)
                    && string_at(source, "/effectiveEncryption") == Some("encrypted")
            }),
            "{field} must address the protected CDF attribute in its original bytes"
        );
        for selector in ["Type", "Binding"] {
            assert!(
                sources.iter().any(|source| {
                    string_at(source, "/location/namespace") == Some("cdfModelTextBytes")
                        && string_at(source, "/location/field") == Some(selector)
                }),
                "{field} uses the exact primary CDF occurrence"
            );
        }
        assert!(
            sources
                .into_iter()
                .all(|source| !is_model_material_name(source)),
            "{field} has no unrelated embedded chunk selector"
        );
    }
    Ok(())
}

#[test]
fn unchanged_material_priority_uses_the_tagged_or_existing_base_material() -> TestResult {
    for (vehicle, library) in [
        ("TEST_Hull_Cdf_Explicit", "Data/Objects/Test/red.mtl"),
        ("TEST_Hull_Cdf_Same", "Data/Objects/Test/base.mtl"),
        ("TEST_Hull_Plain_Finish", "Data/Objects/Test/base.mtl"),
    ] {
        let assembly = assemble_entries(
            vehicle,
            &LoadoutChanges::new(),
            fitted_cdf_paint_vehicle_entries(),
            Some(BASE_MATERIAL),
        )?;
        let report = paint_evidence(&assembly)?;
        assert_eq!(
            string_at(&report, "/appearance/material"),
            Some(library),
            "{vehicle}"
        );
        assert!(
            field_selectors(&report, "/material")?
                .into_iter()
                .all(|source| !is_cdf_material(source)),
            "{vehicle} does not use the CDF material"
        );
    }
    Ok(())
}

#[test]
fn a_distinct_cdf_without_a_material_uses_the_inherited_base() -> TestResult {
    let cdf = FITTED_CDF_DOCUMENT.replace(FITTED_CDF_MATERIAL, "");
    let entries = fitted_cdf_paint_vehicle_entries()
        .into_iter()
        .map(|entry| {
            if entry.path().ends_with("hull_finish.cdf") {
                TestEntry::stored(entry.path(), cdf.as_bytes()).encrypted()
            } else {
                entry
            }
        })
        .collect();
    let assembly = assemble_entries(
        "TEST_Hull_Cdf_Finish",
        &LoadoutChanges::new(),
        entries,
        Some(BASE_MATERIAL),
    )?;
    let report = paint_evidence(&assembly)?;
    assert_eq!(
        string_at(&report, "/appearance/material"),
        Some("Data/Objects/Test/base.mtl"),
        "the distinct CDF has no material and the base remains selected"
    );
    Ok(())
}

#[test]
fn a_missing_cdf_material_target_remains_a_material_issue() -> TestResult {
    let entries = fitted_cdf_paint_vehicle_entries()
        .into_iter()
        .filter(|entry| !entry.path().ends_with("blue.mtl"))
        .collect();
    let assembly = assemble_entries(
        "TEST_Hull_Cdf_Finish",
        &LoadoutChanges::new(),
        entries,
        Some(BASE_MATERIAL),
    )?;
    let report = paint_evidence(&assembly)?;
    assert_eq!(
        string_at(&report, "/appearance/material"),
        None,
        "the selected physical library is missing"
    );
    assert!(
        assembly
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), IssueKind::MaterialLibrary { .. })),
        "the missing authored material is reported"
    );
    Ok(())
}

#[test]
fn a_missing_finish_cdf_still_returns_the_base_geometry() -> TestResult {
    let entries = fitted_cdf_paint_vehicle_entries()
        .into_iter()
        .filter(|entry| !entry.path().ends_with("hull_finish.cdf"))
        .collect();
    let assembly = assemble_entries(
        "TEST_Hull_Cdf_Finish",
        &LoadoutChanges::new(),
        entries,
        Some(BASE_MATERIAL),
    )?;
    let root = assembly.root().ok_or("the fallback hull")?;
    assert_eq!(
        root.geometry(),
        Some(BASE_MODEL),
        "the base hull was loaded"
    );
    assert!(
        assembly.paint_provenance()?.is_none(),
        "the finish was omitted"
    );
    assert!(
        assembly
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), IssueKind::SubGeometryMissing { .. })),
        "the missing finish CDF is reported"
    );
    Ok(())
}

fn is_cdf_material(source: &Value) -> bool {
    string_at(source, "/location/namespace") == Some("cdfModelTextBytes")
        && string_at(source, "/location/field") == Some("Material")
}

fn assert_encrypted_model_source(report: &Value, field: &str) -> TestResult {
    assert_eq!(
        report
            .pointer(&format!("/fieldOrigins/~1{field}/containsEncryptedSource"))
            .and_then(Value::as_bool),
        Some(true),
        "{field} depends on the encrypted name in the model"
    );
    assert!(
        field_selectors(report, &format!("/{field}"))?
            .iter()
            .any(|source| {
                string_at(source, "/scope") == Some("payload")
                    && string_at(source, "/effectiveEncryption") == Some("encrypted")
                    && is_model_material_name(source)
                    && string_at(source, "/location/indices/0") == Some("1")
                    && source.pointer("/location/indices/1").is_none()
            }),
        "the actual second chunk of the encrypted model contributed to {field}"
    );
    Ok(())
}

fn is_model_material_name(source: &Value) -> bool {
    string_at(source, "/location/namespace") == Some("modelChunk")
        && string_at(source, "/location/field") == Some("materialLibrary")
}
