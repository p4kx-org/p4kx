// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutChanges;
use p4kx_testkit::fitted_paint_vehicle_entries;
use serde_json::Value;

use super::{assemble, assemble_entries, field_selectors, paint_evidence, string_at};
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
