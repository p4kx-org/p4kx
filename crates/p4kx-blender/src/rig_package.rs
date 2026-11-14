// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The package of one rigged mesh, the form of a Character and of a creature with a skeleton.

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use crate::mesh_options::package_mesh_options;
use p4kx_extraction::OriginDigestWriter;
use p4kx_gltf::{GlbOptions, GlbSummary, GltfError};

use crate::build::{coordinates, generator};
use crate::finite::sanitise;
use crate::manifest::{
    FORMAT_VERSION, Format, Issue, IssueOrigin, MESHES_FOLDER, Manifest, Mesh, Node, Options,
    Source,
};
use crate::manifest_rig::Rig;
use crate::mesh_vertex_map::Writer;

use crate::package_receipt::{self, PackageFile, PackageTotals};
use crate::plan::mesh_id;
use crate::stage::Staging;
use crate::{PackageError, PackageInfo, PackageResult, PackageSummary, PackageWriteContext};

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
    let mut issues = scene.issues;
    issues.extend(written.texture_fallbacks.iter().map(|fallback| Issue {
        origin: IssueOrigin::Texture,
        kind: "texture".to_owned(),
        path: fallback.file.clone(),
        name: fallback.material.clone(),
        skipped: false,
        reason: fallback.reason.clone(),
    }));
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
            vertex_colours: options.writes_vertex_colours(),
            exclude: info.exclude.clone(),
            replace: info.replace.clone(),
        },
        nodes: vec![node],
        meshes: vec![Mesh {
            file: format!("{MESHES_FOLDER}/{id}.glb"),
            vertex_map: None,
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
        material_libraries: Vec::new(),
        palettes: Vec::new(),
        palette_provenance: None,
        authored_soc: None,
        collision_helpers: None,
        default_material: None,
        entity_provenance: None,
        light_placements: None,
        entity_glow: None,
        issues,
    }
}

/// Stops a write when the caller asks to.
pub(crate) struct Stoppable<'a, W> {
    inner: W,
    cancelled: &'a PackageWriteContext<'a>,
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
pub(crate) type RigWriter<'a, 'sink> =
    Stoppable<'a, OriginDigestWriter<&'sink mut BufWriter<File>>>;

type OptionalMap<'a> = Option<&'a mut Writer>;

/// Writes a package into the folder `out` whose only mesh, `meshes/m0001.glb`, `write_glb` writes
/// to the writer it is given with the GLB options of a package mesh (`options` with the vertex
/// colours of layered surfaces, which the add-on reads as layer shares), and whose manifest `manifest_of` makes from what was written.
///
/// `out` is checked before anything is done and replaced only once the package is complete;
/// `cancelled` is polled while the file is written. `name` names the mesh in an error.
pub(crate) fn write_rig_package(
    out: &Path,
    name: &str,
    options: &GlbOptions,
    cancelled: &PackageWriteContext<'_>,

    write_glb: impl FnOnce(
        &GlbOptions,
        &mut RigWriter<'_, '_>,
        OptionalMap<'_>,
    ) -> Result<GlbSummary, GltfError>,
    manifest_of: impl FnOnce(&GlbSummary) -> Manifest,
) -> PackageResult<PackageSummary> {
    let staging = Staging::create(out, *cancelled)?;
    let result = write_rig_files(
        staging.path(),
        name,
        options,
        cancelled,
        write_glb,
        manifest_of,
    );
    staging.complete(result)
}

fn write_rig_files(
    staging: &Path,
    name: &str,
    options: &GlbOptions,
    cancelled: &PackageWriteContext<'_>,

    write_glb: impl FnOnce(
        &GlbOptions,
        &mut RigWriter<'_, '_>,
        OptionalMap<'_>,
    ) -> Result<GlbSummary, GltfError>,
    manifest_of: impl FnOnce(&GlbSummary) -> Manifest,
) -> PackageResult<PackageSummary> {
    cancelled.check()?;
    let id = mesh_id(0);
    let relative = format!("{MESHES_FOLDER}/{id}.glb");
    let path = staging.join(&relative);
    let map_name = format!("{id}.vertex-map.jsonl");
    let map_path = staging.join(MESHES_FOLDER).join(&map_name);
    let mut map = cancelled
        .vertex_maps
        .then(|| Writer::new(&map_path, cancelled))
        .transpose()?;
    let (file, ownership) = cancelled.create_file(&path)?.into_parts();
    let mut sink = BufWriter::new(file);
    let mut writer = Stoppable {
        inner: OriginDigestWriter::new(&mut sink),
        cancelled,
    };
    let written = write_glb(&package_mesh_options(options), &mut writer, map.as_mut())
        .map_err(|error| glb_error(error, &path, (&id, name), cancelled))?;
    let digest = writer
        .inner
        .finish()
        .map_err(|source| flush_error(source, &path, cancelled))?;
    ownership.ensure_writable(&path, cancelled.cancel_flag())?;
    sink.get_ref()
        .sync_all()
        .map_err(|source| PackageError::write(&path, source))?;
    drop(sink);
    cancelled.check()?;
    let mut manifest = manifest_of(&written);
    let sidecar = map
        .map(|map| map.finish(&map_path, &map_name, &relative, &digest, cancelled))
        .transpose()?;
    if let Some(sidecar) = &sidecar {
        let mesh = manifest.meshes.first_mut().ok_or_else(|| {
            PackageError::write(
                &map_path,
                io::Error::other("rigged mesh is missing from its manifest"),
            )
        })?;
        mesh.vertex_map = Some(sidecar.path.clone());
    }
    sanitise(&mut manifest);
    let files = std::iter::once(PackageFile::new(relative, &digest))
        .chain(sidecar)
        .collect();
    let totals = package_receipt::write_metadata(staging, &manifest, files, cancelled)?;
    let summary = summarise(&manifest, &written, totals);
    cancelled.check()?;
    Ok(summary)
}

fn glb_error(
    error: GltfError,
    path: &Path,
    (id, name): (&str, &str),
    cancelled: &PackageWriteContext<'_>,
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

fn flush_error(
    source: io::Error,
    path: &Path,
    cancelled: &PackageWriteContext<'_>,
) -> PackageError {
    if cancelled.check().is_err() {
        PackageError::Cancelled
    } else {
        PackageError::write(path, source)
    }
}

fn summarise(manifest: &Manifest, written: &GlbSummary, totals: PackageTotals) -> PackageSummary {
    PackageSummary {
        nodes: manifest.nodes.len(),
        drawn_nodes: 1,
        meshes: 1,
        materials: written.materials,
        files: totals.files,
        bytes: totals.bytes,
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
