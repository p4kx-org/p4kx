// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::File;
use std::io::{self, BufWriter, Write};
use std::path::Path;

use p4kx_assembly::{Armour, CharacterAnimations, Issue as AssemblyIssue, PartOrigin};
use p4kx_gltf::{GlbOptions, GlbSummary, GltfError, TextureLoader};

use crate::build::{coordinates, generator};
use crate::finite::sanitise;
use crate::manifest::{
    CharacterSource, EntityRef, FORMAT_VERSION, Format, Issue, IssueOrigin, MANIFEST_FILE,
    MESHES_FOLDER, Manifest, Mesh, Node, NodeKind, Options, Source,
};
use crate::manifest_rig::{Rig, RigClip, RigPart, RigPartOrigin};
use crate::plan::mesh_id;
use crate::stage::Staging;
use crate::write::write_manifest;
use crate::{Cancelled, PackageError, PackageInfo, PackageResult, PackageSummary};

/// A Character ready to be written as a package: its [`Armour`] with the body, head and pieces it
/// wears, and the animations fitted to it by [`Armour::animate`].
#[derive(Debug, Clone, Copy)]
pub struct CharacterScene<'a> {
    name: &'a str,
    armour: &'a Armour,
    animations: &'a CharacterAnimations<'a>,
    chf: Option<&'a str>,
    loadout: Option<&'a str>,
}

impl<'a> CharacterScene<'a> {
    /// Records the CHF file the head came from and the Loadout the pieces came from, as
    /// `source.character` states them; a folder in `chf` is dropped.
    #[must_use]
    pub const fn with_origin(mut self, chf: Option<&'a str>, loadout: Option<&'a str>) -> Self {
        self.chf = chf;
        self.loadout = loadout;
        self
    }

    /// Names the Character `name`, the name of the package's root node, wearing `armour` with
    /// `animations`.
    #[must_use]
    pub const fn new(
        name: &'a str,
        armour: &'a Armour,
        animations: &'a CharacterAnimations<'a>,
    ) -> Self {
        Self {
            name,
            armour,
            animations,
            chf: None,
            loadout: None,
        }
    }
}

/// Stops a write when the caller asks to.
struct Stoppable<'a, W> {
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

/// Writes a Character as a package into the folder `out`.
///
/// The package is the folder of [`write_package`](crate::write_package) with one mesh: `meshes/
/// m0001.glb` is the Character as [`Armour::write_glb_animated_with`] writes it, the skeleton as
/// bone nodes, one skin over them, a skinned mesh per part, the morph targets of the parts and the
/// animations, and the manifest has one node of the kind `character` that draws it. The mesh's
/// `rig` lists the parts, morph targets and animations. The materials of the Character are in the
/// GLB file: plain colours with `options.materials` set to colours, otherwise the textures
/// embedded at `options.texture_mip`; the package has no texture files. `info` supplies the
/// Channel and Build the manifest records.
///
/// `out` is treated as in [`write_package`](crate::write_package): checked before anything is
/// done, written into a temporary folder next to it which replaces it once complete, polled for
/// `cancelled` while the file is written.
///
/// # Errors
///
/// Returns [`PackageError::Refused`] when `out` lies inside a Star Citizen installation,
/// [`PackageError::NotAPackage`] when `out` is something else than an empty folder or a package,
/// [`PackageError::Mesh`] when the Character does not fit in a GLB file,
/// [`PackageError::Write`] when a file or folder cannot be written and
/// [`PackageError::Cancelled`] when `cancelled` returned true.
pub fn write_character_package(
    character: &CharacterScene<'_>,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    cancelled: &(dyn Fn() -> bool + Sync),
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
    let written = character
        .armour
        .write_glb_animated_with(
            character.animations,
            &options.with_layer_vertex_colours(),
            textures,
            &mut writer,
        )
        .map_err(|error| glb_error(error, &path, &id, character.name, &cancelled))?;
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
    let mut manifest = manifest(character, info, options, &written);
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
    id: &str,
    name: &str,
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

fn source(character: &CharacterScene<'_>, info: &PackageInfo) -> Source {
    Source {
        channel: info.channel.clone(),
        build: info.build.clone(),
        entity: EntityRef {
            class_name: character.name.to_owned(),
            guid: String::new(),
        },
        character: Some(CharacterSource {
            name: character.name.to_owned(),
            body: character.armour.variant().code().to_owned(),
            chf: character
                .chf
                .and_then(|chf| Path::new(chf.trim_end_matches(['/', '\\'])).file_name())
                .map(|name| name.to_string_lossy().into_owned()),
            loadout: character.loadout.map(str::to_owned),
        }),
    }
}

fn manifest(
    character: &CharacterScene<'_>,
    info: &PackageInfo,
    options: &GlbOptions,
    written: &GlbSummary,
) -> Manifest {
    let armour = character.armour;
    let id = mesh_id(0);
    Manifest {
        format: Format::P4kxBlenderPackage,
        version: FORMAT_VERSION.to_owned(),
        generator: generator(),
        source: source(character, info),
        coordinates: coordinates(),
        options: Options {
            lod: options.lod,
            texture_mip: options.texture_mip,
            vertex_colours: options.vertex_colours,
            exclude: info.exclude.clone(),
            replace: info.replace.clone(),
        },
        nodes: vec![Node {
            kind: NodeKind::Character,
            name: character.name.to_owned(),
            path: String::new(),
            parent: None,
            entity: None,
            light: None,
            mesh: Some(id.clone()),
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        }],
        meshes: vec![Mesh {
            file: format!("{MESHES_FOLDER}/{id}.glb"),
            id,
            geometry: None,
            palette: None,
            palette_id: None,
            instances: 1,
            bytes: written.bytes,
            vertices: written.vertices,
            triangles: written.triangles,
            materials: Vec::new(),
            rig: Some(rig(armour, written)),
        }],
        textures: Vec::new(),
        materials: Vec::new(),
        palettes: Vec::new(),
        issues: armour
            .issues()
            .iter()
            .chain(character.animations.issues())
            .map(issue)
            .collect(),
    }
}

fn rig(armour: &Armour, written: &GlbSummary) -> Rig {
    Rig {
        body: armour.variant().code().to_owned(),
        skeleton: armour.skeleton_file().to_owned(),
        bones: written.bones,
        skinned_meshes: written.skinned_meshes,
        morph_targets: written.morph_targets,
        embedded_textures: written.textures,
        parts: armour
            .parts()
            .iter()
            .map(|part| RigPart {
                name: part.name().to_owned(),
                origin: match part.origin() {
                    PartOrigin::Body => RigPartOrigin::Body,
                    _ => RigPartOrigin::Piece,
                },
                file: part.file().to_owned(),
                morph_targets: part.morphs().len(),
            })
            .collect(),
        animations: written
            .animations
            .iter()
            .filter(|animation| animation.channels > 0)
            .map(|animation| RigClip {
                name: animation.name.clone(),
                duration_seconds: animation.duration_seconds,
                additive: animation.additive,
                channels: animation.channels,
                keys: animation.keys,
            })
            .collect(),
    }
}

fn issue(issue: &AssemblyIssue) -> Issue {
    Issue {
        origin: IssueOrigin::Assembly,
        kind: issue.kind().code().to_owned(),
        path: issue.path().to_owned(),
        name: issue.name().to_owned(),
        skipped: issue.kind().skips(),
        reason: issue.kind().to_string(),
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
