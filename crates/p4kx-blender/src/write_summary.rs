// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::manifest::NodeKind;
use crate::meshes;
use crate::textures::Outcome;

use super::PackageSummary;

fn count(manifest: &crate::Manifest, kind: NodeKind) -> usize {
    manifest
        .nodes
        .iter()
        .filter(|node| node.kind == kind)
        .count()
}

pub(super) fn summarise(
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
