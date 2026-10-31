// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The package of one rigged mesh, the form of a Character and of a creature with a skeleton.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_gltf::{GlbOptions, GlbSummary, GltfError};

use crate::build::{coordinates, generator};
use crate::finite::sanitise;
use crate::manifest::{
    FORMAT_VERSION, Format, Issue, MANIFEST_FILE, MESHES_FOLDER, Manifest, Mesh, Node, Options,
    Source,
};
use crate::manifest_rig::Rig;
use crate::plan::mesh_id;
use crate::stage::Staging;
use crate::write::write_manifest;
use crate::{Cancelled, PackageError, PackageInfo, PackageResult, PackageSummary};

/// What the manifest of a rigged package says besides the file's size: where it comes from, the
/// node that draws the mesh, the mesh's geometry file and rig, and the issues.
pub(crate) struct RigScene {
    pub(crate) source: Source,
    pub(crate) node: Node,
    pub(crate) geometry: Option<String>,
    pub(crate) rig: Rig,
    pub(crate) issues: Vec<Issue>,
}

/// Returns the manifest of a package with the one rigged mesh `written`, drawn by `scene.node`.
pub(crate) fn rig_manifest(
    scene: RigScene,
    info: &PackageInfo,
    options: &GlbOptions,
    written: &GlbSummary,
) -> Manifest {
    let id = mesh_id(0);
    let mut node = scene.node;
    node.mesh = Some(id.clone());
    Manifest {
        format: Format::P4kxBlenderPackage,
        version: FORMAT_VERSION.to_owned(),
        generator: generator(),
        source: scene.source,
        coordinates: coordinates(),
        options: Options {
            lod: options.lod,
            texture_mip: options.texture_mip,
            vertex_colours: options.vertex_colours,
            exclude: info.exclude.clone(),
            replace: info.replace.clone(),
        },
        nodes: vec![node],
        meshes: vec![Mesh {
            file: format!("{MESHES_FOLDER}/{id}.glb"),
            id,
            geometry: scene.geometry,
            palette: None,
            palette_id: None,
            instances: 1,
            bytes: written.bytes,
            vertices: written.vertices,
            triangles: written.triangles,
            materials: Vec::new(),
            rig: Some(scene.rig),
        }],
        textures: Vec::new(),
        materials: Vec::new(),
        palettes: Vec::new(),
        issues: scene.issues,
    }
}

/// Stops a write when the caller asks to.
pub(crate) struct Stoppable<'a, W> {
    inner: W,
    cancelled: &'a Cancelled<'a>,
}

impl<W: Write> Write for Stoppable<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if self.cancelled.check().is_err() {
            return Err(io::Error::other("cancelled"));
        }
        self.inner.write(bytes)
    }

    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

/// The file a rigged mesh is written to.
pub(crate) type RigWriter<'a> = Stoppable<'a, BufWriter<File>>;

/// Writes a package into the folder `out` whose only mesh, `meshes/m0001.glb`, `write_glb` writes
/// to the writer it is given, and whose manifest `manifest_of` makes from what was written.
///
/// `out` is checked before anything is done and replaced only once the package is complete;
/// `cancelled` is polled while the file is written. `name` names the mesh in an error.
pub(crate) fn write_rig_package(
    out: &Path,
    name: &str,
    cancelled: &(dyn Fn() -> bool + Sync),
    write_glb: impl FnOnce(&mut RigWriter<'_>) -> Result<GlbSummary, GltfError>,
    manifest_of: impl FnOnce(&GlbSummary) -> Manifest,
) -> PackageResult<PackageSummary> {
    let cancelled = Cancelled::new(cancelled);
    let staging = Staging::create(out)?;
    cancelled.check()?;
    let id = mesh_id(0);
    let path = staging.path().join(MESHES_FOLDER).join(format!("{id}.glb"));
    let file = File::create_new(&path).map_err(|source| PackageError::write(&path, source))?;
    let mut writer = Stoppable {
        inner: BufWriter::new(file),
        cancelled: &cancelled,
    };
    let written =
        write_glb(&mut writer).map_err(|error| glb_error(error, &path, (&id, name), &cancelled))?;
    writer
        .flush()
        .map_err(|source| flush_error(source, &path, &cancelled))?;
    let file = writer
        .inner
        .into_inner()
        .map_err(|error| PackageError::write(&path, error.into_error()))?;
    file.sync_all()
        .map_err(|source| PackageError::write(&path, source))?;
    drop(file);
    cancelled.check()?;
    let mut manifest = manifest_of(&written);
    sanitise(&mut manifest);
    let manifest_len = write_manifest(&staging.path().join(MANIFEST_FILE), &manifest)?;
    let summary = summarise(&manifest, &written, manifest_len);
    cancelled.check()?;
    staging.commit()?;
    Ok(summary)
}

fn glb_error(
    error: GltfError,
    path: &Path,
    (id, name): (&str, &str),
    cancelled: &Cancelled<'_>,
) -> PackageError {
    match error {
        GltfError::Write(source) => flush_error(source, path, cancelled),
        source => PackageError::Mesh {
            id: id.to_owned(),
            name: name.to_owned(),
            source,
        },
    }
}

fn flush_error(source: io::Error, path: &Path, cancelled: &Cancelled<'_>) -> PackageError {
    if cancelled.check().is_err() {
        PackageError::Cancelled
    } else {
        PackageError::write(path, source)
    }
}

fn summarise(manifest: &Manifest, written: &GlbSummary, manifest_len: u64) -> PackageSummary {
    PackageSummary {
        nodes: manifest.nodes.len(),
        drawn_nodes: 1,
        meshes: 1,
        materials: written.materials,
        files: 2,
        bytes: manifest_len + written.bytes,
        vertices: written.vertices,
        triangles: written.triangles,
        lod_fallbacks: written.lod_fallbacks,
        bones: written.bones,
        skinned_meshes: written.skinned_meshes,
        morph_targets: written.morph_targets,
        embedded_textures: written.textures,
        clips: written.animations.clone(),
        issues: manifest.issues.clone(),
        ..PackageSummary::default()
    }
}
