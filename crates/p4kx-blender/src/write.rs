// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::sync::atomic::AtomicBool;

use p4kx_assembly::{Assembly, Issue as AssemblyIssue, ModelProvider};
use p4kx_extraction::ArchiveDefaultMaterial;
use p4kx_gltf::{AnimationSummary, GlbOptions, TextureLoader};

use crate::build::{Inputs, manifest};
use crate::collect::Collected;
use crate::creature::write_creature_package;
use crate::entity_classification::EntityClassifications;
use crate::finite::sanitise;
use crate::items::{Item, entity_ref, items};
use crate::manifest::{
    BuildInfo, EntityRef, Issue, MESHES_FOLDER, NodeKind, Replacement, TEXTURES_FOLDER,
};
use crate::package_receipt::{self, PackageFile};
use crate::plan::MeshPlan;
use crate::stage::Staging;
use crate::textures::{self, Outcome};
use crate::{PackageResult, PackageWriteContext, meshes};

/// What the package records about where the Assembly comes from and how it was chosen.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct PackageInfo {
    /// The Channel folder name of the Archive.
    pub channel: Option<String>,
    /// The Build of the Archive.
    pub build: Option<BuildInfo>,
    /// The ports the Loadout leaves out.
    pub exclude: Vec<String>,
    /// The Entities attached to a port instead of what it holds.
    pub replace: Vec<Replacement>,
}

impl PackageInfo {
    /// Creates the information for an Archive in the Channel `channel` with the Build `build`.
    #[must_use]
    pub const fn new(channel: Option<String>, build: Option<BuildInfo>) -> Self {
        Self {
            channel,
            build,
            exclude: Vec::new(),
            replace: Vec::new(),
        }
    }

    /// Records the Loadout changes the Assembly was built with.
    #[must_use]
    pub fn with_changes(mut self, exclude: Vec<String>, replace: Vec<Replacement>) -> Self {
        self.exclude = exclude;
        self.replace = replace;
        self
    }
}

/// What a package consists of.
#[derive(Debug, Clone, Default, PartialEq)]
#[non_exhaustive]
pub struct PackageSummary {
    /// The number of nodes of the scene.
    pub nodes: usize,
    /// The number of nodes of the kind `container`, the Interior's groups.
    pub containers: usize,
    /// The number of nodes of the kind `object`, the models and props of the Interior.
    pub objects: usize,
    /// The number of nodes of the kind `light`.
    pub lights: usize,
    /// The number of nodes that draw a mesh.
    pub drawn_nodes: usize,
    /// The number of GLB meshes, one per distinct mesh set.
    pub meshes: usize,
    /// The number of nodes that draw a mesh another node already draws.
    pub shared_meshes: usize,
    /// The number of PNG textures written.
    pub textures: usize,
    /// The number of pixels of all textures written.
    pub texture_pixels: u64,
    /// The number of textures that could not be written; the materials keep their plain colours.
    pub texture_failures: usize,
    /// The number of material descriptions, layer materials included.
    pub materials: usize,
    /// The number of tint palettes.
    pub palettes: usize,
    /// The number of files in the package, including metadata and optional companions.
    pub files: usize,
    /// The size of all files in bytes.
    pub bytes: u64,
    /// The number of vertices of all meshes, each counted once.
    pub vertices: usize,
    /// The number of triangles of all meshes, each counted once.
    pub triangles: usize,
    /// The number of meshes whose model lacks the level of detail asked for and is written at the
    /// nearest lower one.
    pub lod_fallbacks: usize,
    /// The number of bones of a Character's skeleton; 0 for an Assembly.
    pub bones: usize,
    /// The number of meshes of a Character that carry bone weights.
    pub skinned_meshes: usize,
    /// The number of morph targets of a Character.
    pub morph_targets: usize,
    /// The number of PNG images embedded in a Character's GLB file.
    pub embedded_textures: usize,
    /// What became of each animation asked for, including those left out; empty for an Assembly.
    pub clips: Vec<AnimationSummary>,
    /// The issues the manifest lists.
    pub issues: Vec<Issue>,
}

impl PackageSummary {
    /// Returns the number of animations a Character's file holds: those with at least one channel.
    #[must_use]
    pub fn animations(&self) -> usize {
        self.clips.iter().filter(|clip| clip.channels > 0).count()
    }
}

/// Writes `assembly` as a package into the folder `out`.
///
/// The package is a folder with `scene.json`, one GLB file per distinct mesh below `meshes` and one
/// PNG file per distinct texture below `textures`; the module documentation of the crate and
/// `docs/blender-package.md` describe it. Meshes are drawn with plain colours, with the palette of
/// each part applied; the textures of their materials are written as they are in the Archive, at
/// the mip level `options.texture_mip`, not baked into the meshes. The material descriptions list
/// every texture slot, layer, palette entry and shader parameter of the materials the meshes are
/// drawn with; `models` supplies the layer materials their layers name. The level of detail, the mip
/// level and the vertex colours are those of `options`; `options.materials` is ignored. The
/// Interior of `assembly`, if it has one, is written below an `Interior` node of the root: its
/// containers, models, props and lights are nodes of the kinds `container`, `object` and `light`.
///
/// An Assembly of a creature with its skeleton and a skin bound to it ([`Assembly::creature_skins`]
/// yields a part) is written as one skinned rig mesh instead: `meshes/m0001.glb` holds the skeleton, the skins and the parts on
/// its bones, with their textures embedded, and the one node draws it with a `rig`; `models` is
/// not read then and `options.materials` is ignored as above. Its Interior is left out and listed as
/// the issue `interior_not_written`.
///
/// Everything is written into a temporary folder next to `out`, which replaces `out` once the
/// package is complete. `out` must not exist, or be an empty folder, or hold a package an earlier
/// run wrote; an earlier package stays when the new one fails. `cancelled` is polled between the
/// files and stops the run with [`PackageError::Cancelled`].
///
/// A texture that cannot be loaded does not fail the package: its materials keep their plain
/// colours and the manifest lists it among the issues.
///
/// # Errors
///
/// Returns [`PackageError::Refused`] when `out` lies inside a Star Citizen installation,
/// [`PackageError::NotAPackage`] when `out` is something else than the above,
/// [`PackageError::Mesh`] when a mesh does not fit in a GLB file, [`PackageError::Write`] when a
/// file or folder cannot be written and [`PackageError::Cancelled`] when `cancelled` returned
/// true.
pub fn write_package(
    assembly: &Assembly,
    models: &dyn ModelProvider,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> PackageResult<PackageSummary> {
    let observed = AtomicBool::new(false);
    write_package_with_context(
        assembly,
        models,
        textures,
        options,
        info,
        out,
        &PackageWriteContext::standalone(cancelled, &observed),
    )
}

/// Writes an Assembly package with source protection at every output boundary.
///
/// # Errors
/// Returns protection, cancellation, package validation, mesh or file failures.
pub fn write_package_with_context(
    assembly: &Assembly,
    models: &dyn ModelProvider,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<PackageSummary> {
    if assembly
        .creature_skins()
        .is_some_and(|mut skins| skins.next().is_some())
    {
        return write_creature_package(assembly, textures, options, info, out, context);
    }
    let scene = Scene {
        entity: entity_ref(assembly.root()),
        items: items(assembly),
        issues: assembly.issues(),
        authored_soc: None,
        classification: Some(assembly),
    };
    write_scene(&scene, models, textures, options, info, out, context)
}

/// The nodes of a package before the files are written, and where they come from.
pub(crate) struct Scene<'a> {
    pub(crate) entity: EntityRef,
    pub(crate) items: Vec<Item<'a>>,
    pub(crate) issues: &'a [AssemblyIssue],
    pub(crate) authored_soc: Option<&'a crate::SocScene>,
    pub(crate) classification: Option<&'a Assembly>,
}

pub(crate) fn write_scene(
    scene: &Scene<'_>,
    models: &dyn ModelProvider,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<PackageSummary> {
    let staging = Staging::create(out, *context)?;
    let result = write_scene_files(
        scene,
        models,
        textures,
        options,
        info,
        staging.path(),
        context,
    );
    staging.complete(result)
}

fn write_scene_files(
    scene: &Scene<'_>,
    models: &dyn ModelProvider,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    staging: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<PackageSummary> {
    let items = &scene.items;
    let plan = MeshPlan::of(items);
    let fallback = crate::package_default_material::select(items, &plan, options.lod, models);
    let library = fallback
        .as_deref()
        .and_then(ArchiveDefaultMaterial::library);
    let collected = Collected::of(items, &plan, options.lod, models, library);
    context.check()?;
    let outcomes = if collected.wanted.is_empty() {
        Vec::new()
    } else {
        textures::write_all(
            &collected.wanted,
            textures,
            options.texture_mip,
            &staging.join(TEXTURES_FOLDER),
            context,
        )?
    };
    let written = meshes::write_all(
        items,
        &plan,
        options,
        &staging.join(MESHES_FOLDER),
        context,
        fallback
            .as_deref()
            .and_then(ArchiveDefaultMaterial::material),
    )?;
    context.check()?;
    let mut manifest = manifest(&Inputs {
        entity: scene.entity.clone(),
        assembly_issues: scene.issues,
        items,
        info,
        options,
        plan: &plan,
        collected: &collected,
        outcomes: &outcomes,
        meshes: &written,
    });
    let authored_soc_file =
        complete_manifest(&mut manifest, scene, fallback.as_deref(), staging, context)?;
    let mut summary = summarise(&manifest, &outcomes, &written);
    let files = package_files(written, &outcomes, authored_soc_file);
    let totals = package_receipt::write_metadata(staging, &manifest, files, context)?;
    summary.files = totals.files;
    summary.bytes = totals.bytes;
    context.check()?;
    Ok(summary)
}

fn complete_manifest(
    manifest: &mut crate::Manifest,
    scene: &Scene<'_>,
    fallback: Option<&ArchiveDefaultMaterial>,
    staging: &Path,
    context: &PackageWriteContext<'_>,
) -> PackageResult<Option<PackageFile>> {
    let authored_soc = scene
        .authored_soc
        .map(|authored| crate::package_soc_scene::write(staging, authored, context))
        .transpose()?;
    if authored_soc.is_some() {
        manifest.authored_soc = Some(crate::package_soc_scene::ManifestSocScene::new());
    }
    manifest.default_material = fallback.map(ArchiveDefaultMaterial::receipt);
    if let Some(assembly) = scene.classification {
        EntityClassifications::capture(assembly.parts(), assembly.forge_origin())?.apply(manifest);
        manifest.light_placements =
            crate::package_light_placements::capture(assembly, &scene.items, context)?;
    }
    sanitise(manifest);
    Ok(authored_soc)
}

fn package_files(
    meshes: Vec<meshes::WrittenMesh>,
    outcomes: &[Outcome],
    authored_soc: Option<PackageFile>,
) -> Vec<PackageFile> {
    meshes
        .into_iter()
        .map(|mesh| mesh.file)
        .chain(
            outcomes
                .iter()
                .enumerate()
                .flat_map(|(position, outcome)| textures::package_files(position, outcome)),
        )
        .chain(authored_soc)
        .collect()
}

fn count(manifest: &crate::Manifest, kind: NodeKind) -> usize {
    manifest
        .nodes
        .iter()
        .filter(|node| node.kind == kind)
        .count()
}

fn summarise(
    manifest: &crate::Manifest,
    outcomes: &[Outcome],
    meshes: &[meshes::WrittenMesh],
) -> PackageSummary {
    let mut summary = PackageSummary {
        nodes: manifest.nodes.len(),
        containers: count(manifest, NodeKind::Container),
        objects: count(manifest, NodeKind::Object),
        lights: count(manifest, NodeKind::Light),
        drawn_nodes: manifest
            .nodes
            .iter()
            .filter(|node| node.mesh.is_some())
            .count(),
        meshes: manifest.meshes.len(),
        textures: manifest.textures.len(),
        materials: manifest.materials.len(),
        palettes: manifest.palettes.len(),
        issues: manifest.issues.clone(),
        ..PackageSummary::default()
    };
    summary.shared_meshes = summary.drawn_nodes.saturating_sub(summary.meshes);
    for mesh in meshes {
        summary.vertices += mesh.summary.vertices;
        summary.triangles += mesh.summary.triangles;
        summary.lod_fallbacks += mesh.summary.lod_fallbacks;
    }
    for outcome in outcomes {
        match outcome {
            Outcome::Written {
                width,
                height,
                smoothness,
                ..
            } => {
                let images = if smoothness.is_some() { 2 } else { 1 };
                summary.texture_pixels += images * u64::from(*width) * u64::from(*height);
            }
            Outcome::Failed { .. } => summary.texture_failures += 1,
        }
    }
    summary
}
