// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gltf::AnimationSummary;

use crate::manifest::{Issue, Manifest, NodeKind};
use crate::meshes;
use crate::textures::Outcome;

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

fn count(manifest: &Manifest, kind: NodeKind) -> usize {
    manifest
        .nodes
        .iter()
        .filter(|node| node.kind == kind)
        .count()
}

pub(crate) fn summarise(
    manifest: &Manifest,
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
