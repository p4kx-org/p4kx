// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::OriginDigestWriter;
use p4kx_testkit::{TestEntry, assembly_entries, test_key};
use serde_json::{Value, json};

use super::{TestResult, hologram, skip, write_archive};

fn fixture(encrypt_selected: bool) -> Vec<TestEntry> {
    assembly_entries()
        .into_iter()
        .map(|entry| {
            let path = entry.path();
            if path.ends_with("ship_lod1.cgfm")
                || path.ends_with("cube.mtl")
                || (encrypt_selected
                    && (path.ends_with("ship.cgfm") || path.ends_with("gun_paint.mtl")))
            {
                entry.encrypted()
            } else {
                entry
            }
        })
        .collect()
}

fn receipt(out: &Path, report: &Value) -> Result<Vec<Value>, Box<dyn std::error::Error>> {
    assert_eq!(
        report["originReceipt"], "ship.png.p4kx-origin.jsonl",
        "sibling basename"
    );
    let file = out.with_file_name("ship.png.p4kx-origin.jsonl");
    let rows: Vec<Value> = fs::read_to_string(file)?
        .lines()
        .map(serde_json::from_str)
        .collect::<Result<_, _>>()?;
    let bytes = fs::read(out)?;
    let mut measured = OriginDigestWriter::new(io::sink());
    measured.write_all(&bytes)?;
    let digest = measured.finish()?;
    assert_eq!(
        report["bytes"].as_u64(),
        Some(digest.byte_length()),
        "same measured length"
    );
    let digest = serde_json::to_value(digest)?;
    let binding = rows
        .first()
        .and_then(|row| row.get("binding"))
        .ok_or("receipt binding")?;
    assert_eq!(
        binding.pointer("/path/value"),
        Some(&json!("ship.png")),
        "bound artifact name"
    );
    for field in ["sha256", "byteLength"] {
        let expected = digest.get(field).ok_or("measured digest field")?;
        assert_eq!(binding.get(field), Some(expected), "exact {field}");
    }
    let datum = rows
        .iter()
        .find(|row| row["type"] == "field")
        .ok_or("receipt datum")?;
    assert_eq!(
        datum.pointer("/address/type"),
        Some(&json!("opaquePayload")),
        "encoded image address"
    );
    let artifact = report.get("artifactOrigin").ok_or("JSON artifact origin")?;
    for field in [
        "containsEncryptedSource",
        "sourceUsesRefs",
        "sourceSetComplete",
    ] {
        let expected = artifact.get(field).ok_or("JSON artifact field")?;
        assert_eq!(datum.get(field), Some(expected), "same artifact {field}");
    }
    assert_eq!(
        rows.last().ok_or("receipt trailer")?["type"],
        "end",
        "complete receipt stream"
    );
    Ok(rows)
}

fn source_uses<'a>(rows: &'a [Value], report: &Value, filename: &str) -> Vec<&'a Value> {
    let source = rows
        .iter()
        .filter_map(|row| row.get("source"))
        .find(|source| {
            source
                .pointer("/path/value")
                .and_then(Value::as_str)
                .is_some_and(|path| path.ends_with(filename))
        });
    let Some(id) = source.and_then(|source| source.get("id")) else {
        return Vec::new();
    };
    rows.iter()
        .filter_map(|row| row.get("sourceUse"))
        .filter(|row| row.get("source") == Some(id))
        .filter(|row| {
            report
                .pointer("/artifactOrigin/sourceUsesRefs")
                .and_then(Value::as_array)
                .is_some_and(|refs| row.get("id").is_some_and(|id| refs.contains(id)))
        })
        .collect()
}

#[test]
fn only_the_selected_models_and_materials_affect_the_bound_png() -> TestResult {
    let dir = tempfile::tempdir()?;
    let key = test_key().to_hex();
    let out = dir.path().join("ship.png");
    let mut baseline = None;
    for encrypted in [true, false] {
        let input = tempfile::tempdir_in(dir.path())?;
        let archive = write_archive(input.path(), fixture(encrypted))?;
        let run = hologram(&archive, &out, &["--size", "64", "--key", &key], true)?;
        if skip(
            "only_the_selected_models_and_materials_affect_the_bound_png",
            &run,
        ) {
            return Ok(());
        }
        assert_eq!(run.code, Some(0), "{}", run.stderr);
        let report: Value = serde_json::from_str(&run.stdout)?;
        let rows = receipt(&out, &report)?;
        let marker = encrypted.then_some(true);
        assert_eq!(
            report["artifactOrigin"]["containsEncryptedSource"],
            json!(marker)
        );
        assert_eq!(report["artifactOrigin"]["sourceSetComplete"], false);
        assert_eq!(report["fieldOrigins"]["/bytes"], report["artifactOrigin"]);
        assert_eq!(
            report["fieldOrigins"]["/width"]["containsEncryptedSource"],
            false
        );
        selected_inputs(&rows, &report);
        let bytes = fs::read(&out)?;
        if let Some(previous) = baseline {
            assert_eq!(bytes, previous, "only source encryption changed");
        }
        baseline = Some(bytes);
    }
    Ok(())
}

fn selected_inputs(rows: &[Value], report: &Value) {
    assert_eq!(
        source_uses(rows, report, "ship_lod1.cgfm"),
        [] as [&Value; 0],
        "unused LOD"
    );
    assert_eq!(
        source_uses(rows, report, "cube.mtl"),
        [] as [&Value; 0],
        "overridden material"
    );
    let geometry = source_uses(rows, report, "ship.cgfm");
    assert!(
        geometry.iter().any(|row| row["role"] == "payload"),
        "the visible geometry supplies the artifact"
    );
    let library = source_uses(rows, report, "gun_paint.mtl");
    assert!(
        library.iter().any(|row| {
            row["location"]
                == json!({
                    "type": "subresource",
                    "namespace": "materialLibrary",
                    "indices": ["2"],
                    "field": "selectedMaterial"
                })
        }),
        "the resolver bound the actual material slot"
    );
    let forge = source_uses(rows, report, "Game2.dcb");
    for selector in [
        ".Geometry.Geometry.Geometry.path",
        ".Geometry.Geometry.Material.path",
    ] {
        assert!(
            forge.iter().any(|row| row["role"] == "selector"
                && row
                    .pointer("/location/field")
                    .and_then(Value::as_str)
                    .is_some_and(|field| field.ends_with(selector))),
            "the actual authored selector {selector}"
        );
    }
    assert!(
        forge.iter().any(|row| {
            row.pointer("/location/field")
                .and_then(Value::as_str)
                .is_some_and(|field| {
                    field.ends_with(".AttachmentImplementation.Helper.Helper.Offset.Rotation")
                })
        }),
        "the placement retains its authored port rotation"
    );
}

#[path = "ui_hologram_material_suppression.rs"]
mod suppression;

#[path = "ui_hologram_model_nodes.rs"]
mod model_nodes;

#[path = "ui_hologram_mesh_nodes.rs"]
mod mesh_nodes;

#[path = "ui_hologram_bone_placement.rs"]
mod bone_placement;

#[path = "ui_hologram_root_rig.rs"]
mod root_rig;

#[path = "ui_hologram_cdf_attachments.rs"]
mod cdf_attachments;

#[path = "ui_hologram_gear_selectors.rs"]
mod gear_selectors;

#[path = "ui_hologram_gear_deploy.rs"]
mod gear_deploy;
