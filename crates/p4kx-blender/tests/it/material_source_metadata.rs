// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_blender::{FORMAT_VERSION, Manifest};

use crate::package_materials;
use crate::support::TestResult;

#[test]
fn package_source_roots_are_shared_without_overriding_original_material_indices() -> TestResult {
    let written = package_materials::write()?;
    let manifest = &written.manifest;
    assert_eq!(
        manifest.version, FORMAT_VERSION,
        "producer format follows the current additive contract"
    );
    assert!(
        !manifest.material_libraries.is_empty(),
        "parsed library roots are exported independently of child descriptions"
    );
    for material in &manifest.materials {
        let index = material
            .source_library
            .ok_or("material source-library link")?;
        let source = manifest
            .material_libraries
            .get(index)
            .ok_or("source-library ordinal")?;
        assert!(
            source.authored.root.is_some(),
            "synthetic source root remains structured"
        );
        if let Some(index) = material.index {
            assert!(
                source
                    .authored
                    .entries
                    .iter()
                    .any(|entry| entry.ordinal == index),
                "rendered submaterial keeps its actual original source position"
            );
        }
    }
    let records = manifest.material_libraries.iter().enumerate();
    for (index, source) in records {
        let later = manifest
            .material_libraries
            .get(index + 1..)
            .ok_or("remaining source descriptions")?;
        assert!(
            !later
                .iter()
                .any(|other| source.path == other.path && source.authored == other.authored),
            "equivalent roots are not cloned per material"
        );
    }
    Ok(())
}

#[test]
fn older_scene_descriptions_without_source_roots_and_links_remain_readable() -> TestResult {
    let written = package_materials::write()?;
    let mut value = serde_json::to_value(&written.manifest)?;
    let root = value.as_object_mut().ok_or("manifest object")?;
    root.remove("material_libraries");
    root.insert("version".into(), serde_json::json!("1.8"));
    let materials = root
        .get_mut("materials")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("materials")?;
    for material in materials {
        material
            .as_object_mut()
            .ok_or("material object")?
            .remove("source_library");
    }
    let legacy: Manifest = serde_json::from_value(value)?;
    assert!(
        legacy.material_libraries.is_empty(),
        "missing additive root collection defaults to empty"
    );
    assert!(
        legacy
            .materials
            .iter()
            .all(|material| material.source_library.is_none()),
        "legacy descriptions do not fabricate source links"
    );
    assert_eq!(
        legacy.materials.len(),
        written.manifest.materials.len(),
        "existing material records are unchanged"
    );
    Ok(())
}

#[test]
fn older_source_rows_without_the_optional_receipt_remain_readable() -> TestResult {
    let written = package_materials::write()?;
    let mut value = serde_json::to_value(&written.manifest)?;
    let libraries = value
        .get_mut("material_libraries")
        .and_then(serde_json::Value::as_array_mut)
        .ok_or("authored library rows")?;
    for library in libraries {
        library
            .as_object_mut()
            .ok_or("library object")?
            .remove("authored_origin");
    }
    let old: Manifest = serde_json::from_value(value)?;
    assert!(
        old.material_libraries
            .iter()
            .all(|library| library.authored_origin.is_none()),
        "older producers can leave out the authored material companion"
    );
    Ok(())
}
