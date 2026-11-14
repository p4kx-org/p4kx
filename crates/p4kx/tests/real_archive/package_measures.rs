// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Error;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::package_fault::{self, Check, Fault};
use crate::package_types::{Built, Measured};

fn array<'a>(value: &'a Value, key: &str) -> package_fault::Result<&'a Vec<Value>> {
    Ok(value[key].as_array().ok_or(Check::ManifestField)?)
}

fn files_below(folder: &Path) -> package_fault::Result<Vec<(PathBuf, u64)>> {
    let mut found = Vec::new();
    let mut pending = vec![folder.to_owned()];
    while let Some(next) = pending.pop() {
        for entry in std::fs::read_dir(next)? {
            let entry = entry?;
            let meta = entry.metadata()?;
            if meta.is_dir() {
                pending.push(entry.path());
            } else {
                found.push((entry.path(), meta.len()));
            }
        }
    }
    Ok(found)
}

#[path = "package_texture_measures.rs"]
mod texture_measures;

pub(crate) fn verify(built: &Built) -> package_fault::Result<Measured> {
    let validator = jsonschema::draft202012::new(&p4kx_blender::manifest_schema())?;
    if let Err(error) = validator.validate(&built.manifest) {
        return Err(Fault::at(Check::Schema, error.to_owned()));
    }
    let on_disk = files_below(&built.out)?;
    let bytes = on_disk.iter().try_fold(0_u64, |total, (_, length)| {
        total.checked_add(*length).ok_or(Check::CountOverflow)
    })?;
    let mut measured = Measured {
        files: on_disk.len(),
        bytes,
        mesh_bytes: 0,
        meshes: 0,
        texture_sources: texture_measures::verify(built)?,
        textures: array(&built.manifest, "textures")?.len(),
        instances: 0,
        undeduplicated_mesh_bytes: 0,
    };
    for mesh in array(&built.manifest, "meshes")? {
        measure_mesh(built, mesh, &mut measured)?;
    }
    for (key, counted) in [("files", measured.files as u64), ("bytes", bytes)] {
        package_fault::require(
            built.report.get(key).and_then(Value::as_u64) == Some(counted),
            Check::ReportCount,
        )?;
    }
    report(built, &measured)?;
    Ok(measured)
}

fn measure_mesh(built: &Built, mesh: &Value, measured: &mut Measured) -> package_fault::Result<()> {
    let file = mesh["file"].as_str().ok_or(Check::ManifestField)?;
    let content = std::fs::read(built.out.join(file))?;
    p4kx_testkit::check_glb(&content)
        .map_err(|error| Fault::at(Check::Glb, Error::other(error)))?;
    let length = content.len() as u64;
    package_fault::require(mesh["bytes"].as_u64() == Some(length), Check::MeshBytes)?;
    let instances = mesh["instances"].as_u64().ok_or(Check::ManifestField)?;
    package_fault::require(instances >= 1, Check::MeshInstances)?;
    measured.meshes = measured.meshes.checked_add(1).ok_or(Check::CountOverflow)?;
    measured.mesh_bytes = measured
        .mesh_bytes
        .checked_add(length)
        .ok_or(Check::CountOverflow)?;
    measured.instances = measured
        .instances
        .checked_add(instances)
        .ok_or(Check::CountOverflow)?;
    let total = length.checked_mul(instances).ok_or(Check::CountOverflow)?;
    measured.undeduplicated_mesh_bytes = measured
        .undeduplicated_mesh_bytes
        .checked_add(total)
        .ok_or(Check::CountOverflow)?;
    Ok(())
}

#[expect(
    clippy::print_stdout,
    reason = "only aggregate package measurements are reported"
)]
fn report(built: &Built, measured: &Measured) -> package_fault::Result<()> {
    println!(
        "package totals: files={} bytes={} meshes={} instances={} sharing={:.3} mesh_bytes={} mesh_ratio={:.3} textures={} texture_sources={} issues={} duration_ms={}",
        measured.files,
        measured.bytes,
        measured.meshes,
        measured.instances,
        measured.sharing(),
        measured.mesh_bytes,
        measured.mesh_ratio(),
        measured.textures,
        measured.texture_sources,
        array(&built.manifest, "issues")?.len(),
        built.elapsed.as_millis()
    );
    Ok(())
}

pub(crate) fn nodes_of(manifest: &Value, kind: &str) -> package_fault::Result<usize> {
    Ok(array(manifest, "nodes")?
        .iter()
        .filter(|node| node["kind"] == kind)
        .count())
}
