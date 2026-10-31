// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! A creature with its skeleton as a package: one skinned rig mesh.

use std::path::Path;

use p4kx_assembly::{Assembly, Part};
use p4kx_gltf::{GlbOptions, GlbSummary, MaterialMode, TextureLoader};

use crate::build::assembly_issue;
use crate::manifest::{EntityRef, Manifest, Node, NodeKind, Source};
use crate::manifest_rig::{Rig, RigPart, RigPartOrigin};
use crate::rig_package::{RigScene, rig_manifest, write_rig_package};
use crate::{PackageInfo, PackageResult, PackageSummary};

/// Writes the creature `assembly` as a package: `meshes/m0001.glb` is the Assembly as
/// [`Assembly::write_glb_with`] writes it, its skeleton as bone nodes with the skins bound to it,
/// and the parts that hang on its bones beside them. The textures are embedded in the file at
/// `options.texture_mip`, as for a Character; the package has no texture files. Expects
/// [`Assembly::creature_skins`] to be `Some`.
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
    write_rig_package(
        out,
        name,
        cancelled,
        |writer| assembly.write_glb_with(&options, textures, writer),
        |written| manifest(assembly, info, &options, written),
    )
}

fn manifest(
    assembly: &Assembly,
    info: &PackageInfo,
    options: &GlbOptions,
    written: &GlbSummary,
) -> Manifest {
    let root = assembly.root();
    let scene = RigScene {
        source: Source {
            channel: info.channel.clone(),
            build: info.build.clone(),
            entity: entity_of(root),
            character: None,
        },
        node: Node {
            kind: NodeKind::Entity,
            name: root.map_or_else(String::new, |root| root.name().to_owned()),
            path: root.map_or_else(String::new, |root| root.path().to_owned()),
            parent: None,
            entity: Some(entity_of(root)),
            light: None,
            mesh: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        },
        geometry: root.and_then(Part::geometry).map(str::to_owned),
        rig: rig(assembly, written),
        issues: assembly.issues().iter().map(assembly_issue).collect(),
    };
    rig_manifest(scene, info, options, written)
}

fn entity_of(root: Option<&Part>) -> EntityRef {
    root.map_or_else(
        || EntityRef {
            class_name: String::new(),
            guid: String::new(),
        },
        |root| EntityRef {
            class_name: root.class_name().to_owned(),
            guid: root.guid().to_string(),
        },
    )
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
