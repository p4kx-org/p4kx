// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{IssueKind, LoadoutChanges};
use p4kx_model::MaterialStatus;
use p4kx_testkit::{fitted_paint_vehicle_entries, fitted_same_model_paint_vehicle_entries};
use serde_json::Value;

use super::{
    BASE_MATERIAL, BASE_MODEL, assemble, assemble_entries, field_of, field_selectors,
    is_model_material_name, paint_evidence, string_at,
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
