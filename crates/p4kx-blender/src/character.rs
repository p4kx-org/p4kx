// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_assembly::{Armour, CharacterAnimations, PartOrigin};
use p4kx_gltf::{GlbOptions, GlbSummary, TextureLoader};

use crate::build::assembly_issue;
use crate::manifest::{CharacterSource, EntityRef, Manifest, Node, NodeKind, Source};
use crate::manifest_rig::{Rig, RigClip, RigPart, RigPartOrigin};
use crate::rig_package::{RigScene, rig_manifest, write_rig_package};
use crate::{PackageInfo, PackageResult, PackageSummary};

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
    write_rig_package(
        out,
        character.name,
        cancelled,
        |writer| {
            character.armour.write_glb_animated_with(
                character.animations,
                options,
                textures,
                writer,
            )
        },
        |written| manifest(character, info, options, written),
    )
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
    let scene = RigScene {
        source: source(character, info),
        node: Node {
            kind: NodeKind::Character,
            name: character.name.to_owned(),
            path: String::new(),
            parent: None,
            entity: None,
            light: None,
            mesh: None,
            translation: [0.0; 3],
            rotation: [0.0, 0.0, 0.0, 1.0],
            scale: [1.0; 3],
        },
        geometry: None,
        rig: rig(armour, written),
        issues: armour
            .issues()
            .iter()
            .chain(character.animations.issues())
            .map(assembly_issue)
            .collect(),
    };
    rig_manifest(scene, info, options, written)
}

fn rig(armour: &Armour, written: &GlbSummary) -> Rig {
    Rig {
        body: Some(armour.variant().code().to_owned()),
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
