// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{BufRead as _, BufReader};
use std::path::Path;

use flate2::bufread::GzDecoder;
use p4kx_blender::{FORMAT_VERSION, Manifest, MaterialLibrarySourceDescription};
use serde_json::Value;

use crate::package_materials;
use crate::support::TestResult;

pub(crate) fn read_rows(path: &Path) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    BufReader::new(GzDecoder::new(BufReader::new(File::open(path)?)))
        .lines()
        .map(|line| Ok(serde_json::from_str(&line?)?))
        .collect()
}

pub(crate) fn row_type(row: &Value) -> Option<&str> {
    row.get("type").and_then(Value::as_str)
}

pub(crate) fn check_field_paths(authored: &Value, rows: &[Value]) {
    for path in rows
        .iter()
        .filter(|row| row_type(row) == Some("field"))
        .filter_map(|row| row.get("path").and_then(Value::as_str))
    {
        assert!(
            authored.pointer(path).is_some(),
            "a field path targets an emitted value {path}"
        );
    }
}

fn library_payload(
    root: &Path,
    source: &MaterialLibrarySourceDescription,
) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    assert!(
        source.authored.is_none() && source.authored_origin.is_none(),
        "authored data is written only once"
    );
    let file = source
        .authored_file
        .as_ref()
        .ok_or("authored source file")?;
    read_rows(&root.join(&file.file))
}

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
    let root = written.dir.path().join("ship");
    let libraries: Vec<_> = manifest
        .material_libraries
        .iter()
        .map(|source| library_payload(&root, source))
        .collect::<Result<_, _>>()?;
    for material in &manifest.materials {
        let index = material
            .source_library
            .ok_or("material source-library link")?;
        let rows = libraries.get(index).ok_or("source-library ordinal")?;
        let source = rows
            .iter()
            .find(|row| row_type(row) == Some("library"))
            .ok_or("library source payload")?;
        assert!(
            source
                .pointer("/authored/root")
                .is_some_and(|root| !root.is_null()),
            "synthetic source root remains structured"
        );
        if let Some(index) = material.index {
            let entries = source
                .pointer("/authored/entries")
                .and_then(Value::as_array)
                .ok_or("original submaterial entries")?;
            assert!(entries.iter().any(|entry| entry.get("ordinal").and_then(Value::as_u64) == Some(index as u64)), "rendered submaterial keeps its original source position");
        }
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
        .and_then(Value::as_array_mut)
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
    let sources = value
        .get_mut("material_libraries")
        .and_then(Value::as_array_mut)
        .ok_or("authored library rows")?;
    for source in sources {
        let object = source.as_object_mut().ok_or("library object")?;
        object.remove("authored_file");
        object.insert(
            "authored".into(),
            serde_json::json!({
                "encoding": "text", "root": null, "losses": [], "entries": []
            }),
        );
    }
    let old: Manifest = serde_json::from_value(value)?;
    assert!(
        old.material_libraries
            .iter()
            .all(|library| library.authored.is_some() && library.authored_file.is_none()),
        "older producers can leave out the authored material companion"
    );
    Ok(())
}
