// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_blender::Manifest;
use p4kx_testkit::check_glb;
use serde_json::{Value, json};

use crate::support::TestResult;

pub(crate) fn assert_mesh_counts(manifest: &Manifest, package: &Path) -> TestResult {
    let scene = manifest.scene_provenance.as_ref().ok_or("scene evidence")?;
    for (slot, mesh) in manifest.meshes.iter().enumerate() {
        let emitted = check_glb(&fs::read(package.join(&mesh.file))?)?;
        let mut triangles = 0;
        for part in emitted.document.meshes() {
            for primitive in part.primitives() {
                triangles += primitive.indices().ok_or("written index accessor")?.count() / 3;
            }
        }
        assert_eq!(
            mesh.triangles, triangles,
            "mesh {slot} counts its actual GLB primitives"
        );
        let origin = triangle_origin(scene, slot)?;
        assert_eq!(
            origin.get("sourceSetComplete"),
            Some(&json!(false)),
            "mesh {slot} has partial selection evidence"
        );
        for count in count_operands(scene, slot)? {
            assert_eq!(
                count.get("role"),
                Some(&json!("conversion")),
                "mesh {slot} count uses"
            );
            assert_eq!(
                count.pointer("/location/field"),
                Some(&json!("indexCount")),
                "mesh {slot} physical count word"
            );
        }
    }
    Ok(())
}

pub(crate) fn triangle_origin(scene: &Value, slot: usize) -> Result<&Value, Box<dyn Error>> {
    scene
        .get("fieldOrigins")
        .and_then(|fields| fields.get(format!("/meshes/{slot}/triangles")))
        .ok_or_else(|| format!("addressed triangles for mesh {slot}").into())
}

pub(crate) fn count_operands(scene: &Value, slot: usize) -> Result<Vec<&Value>, Box<dyn Error>> {
    let references = triangle_origin(scene, slot)?
        .get("sourceUsesRefs")
        .and_then(Value::as_array)
        .ok_or("triangle source references")?;
    let recorded = scene
        .pointer("/sourceInventory/uses")
        .and_then(Value::as_array)
        .ok_or("shared scene source uses")?;
    references
        .iter()
        .map(|reference| {
            recorded
                .iter()
                .find(|row| row.get("id") == Some(reference))
                .ok_or_else(|| "triangle use must resolve in the same scene inventory".into())
        })
        .collect()
}

pub(crate) fn source_entry<'scene>(
    scene: &'scene Value,
    path: &str,
) -> Result<&'scene Value, Box<dyn Error>> {
    let expected = path.replace('\\', "/");
    scene
        .pointer("/sourceInventory/sources")
        .and_then(Value::as_array)
        .and_then(|sources| {
            sources.iter().find(|source| {
                source
                    .pointer("/path/value")
                    .and_then(Value::as_str)
                    .is_some_and(|actual| actual.replace('\\', "/") == expected)
            })
        })
        .ok_or_else(|| format!("scene inventory is missing physical path {path}").into())
}
