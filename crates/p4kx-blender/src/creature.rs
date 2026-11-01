// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A creature with its skeleton as a package: one skinned rig mesh.

use std::path::Path;

use p4kx_assembly::{Assembly, Part};
use p4kx_gltf::{GlbOptions, GlbSummary, MaterialMode, TextureLoader};

use crate::build::assembly_issue;
use crate::items::entity_ref;
use crate::manifest::{Issue, IssueOrigin, Manifest, Node, NodeKind, Source};
use crate::manifest_rig::{Rig, RigPart, RigPartOrigin};
use crate::rig_package::{RigScene, rig_manifest, write_rig_package};
use crate::{PackageInfo, PackageResult, PackageSummary};

/// Writes the creature `assembly` as a package: `meshes/m0001.glb` is the Assembly as
/// [`Assembly::write_glb_with`] writes it, its skeleton as bone nodes with the skins bound to it,
/// and the parts that hang on its bones beside them. The textures are embedded in the file at
/// `options.texture_mip`, as for a Character; the package has no texture files. An Interior of
/// the Assembly is left out and listed as the issue `interior_not_written`. Expects
/// [`Assembly::creature_skins`] to yield a part.
pub(crate) fn write_creature_package(
    assembly: &Assembly,
    textures: &dyn TextureLoader,
    options: &GlbOptions,
    info: &PackageInfo,
    out: &Path,
    cancelled: &(dyn Fn() -> bool + Sync),
) -> PackageResult<PackageSummary> {
    let options = options.with_materials(MaterialMode::Textures);
    let name = assembly.root().map_or("", Part::class_name);
    let interior_left_out = assembly.interior().is_some();
    let stripped;
    let assembly = if interior_left_out {
        stripped = assembly.clone().without_interior();
        &stripped
    } else {
        assembly
    };
    write_rig_package(
        out,
        name,
        &options,
        cancelled,
        |options, writer| assembly.write_glb_with(options, textures, writer),
        |written| manifest(assembly, interior_left_out, info, &options, written),
    )
}

fn manifest(
    assembly: &Assembly,
    interior_left_out: bool,
    info: &PackageInfo,
    options: &GlbOptions,
    written: &GlbSummary,
) -> Manifest {
    let root = assembly.root();
    let entity = entity_ref(root);
    let mut issues: Vec<Issue> = assembly.issues().iter().map(assembly_issue).collect();
    if interior_left_out {
        issues.push(Issue {
            origin: IssueOrigin::Assembly,
            kind: "interior_not_written".to_owned(),
            path: String::new(),
            name: entity.class_name.clone(),
            skipped: true,
            reason: "the Interior of a creature with a skeleton is not written to a rig mesh"
                .to_owned(),
        });
    }
    let scene = RigScene {
        source: Source {
            channel: info.channel.clone(),
            build: info.build.clone(),
            entity: entity.clone(),
            character: None,
        },
        node: Node {
            kind: NodeKind::Entity,
            name: root.map_or_else(String::new, |root| root.name().to_owned()),
            path: root.map_or_else(String::new, |root| root.path().to_owned()),
            parent: None,
            entity: Some(entity),
            light: None,
            mesh: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        },
        geometry: root.and_then(Part::geometry).map(str::to_owned),
        rig: rig(assembly, written),
        issues,
    };
    rig_manifest(scene, info, options, written)
}

fn rig(assembly: &Assembly, written: &GlbSummary) -> Rig {
    Rig {
        body: None,
        skeleton: assembly
            .root()
            .and_then(Part::geometry)
            .unwrap_or_default()
            .to_owned(),
        bones: written.bones,
        skinned_meshes: written.skinned_meshes,
        morph_targets: written.morph_targets,
        embedded_textures: written.textures,
        parts: assembly
            .creature_skins()
            .into_iter()
            .flatten()
            .map(|part| RigPart {
                name: part.name().to_owned(),
                origin: RigPartOrigin::Skin,
                file: part.geometry().unwrap_or_default().to_owned(),
                morph_targets: 0,
            })
            .collect(),
        animations: Vec::new(),
    }
}
