// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The Character assembly of armour: a default body of a [`BodyVariant`] with its skeleton and
//! the [`Piece`]s of armour and clothing worn on it.

mod anchor;
mod animate;
mod body;
mod body_zones;
mod build;
mod dress;
mod expressions;
mod face_build;
mod facial;
mod flat_layers;
mod follow;
mod follow_joints;
mod follow_morphs;
mod forge_items;
mod head;
mod head_move;
mod hidden;
mod jobs;
mod loadout;
mod lod_morphs;
mod look;
mod materials;
mod mount;
mod normals;
mod npc;
mod own_face;
mod piece;
mod read_models;
mod sample;
mod shape;
mod uncover;
mod unused;
mod vec3;
mod wearable;
mod zones;

use std::io::Write;
use std::sync::Arc;

use p4kx_gltf::{
    GlbOptions, GlbSummary, GltfError, MorphTarget, RigMount, RigPiece, TextureLoader,
    write_rigged_glb, write_rigged_glb_with,
};
use p4kx_model::{MaterialStatus, Model};

pub use animate::{CharacterAnimations, MAX_ANIMATION_VALUES};
pub use body::BodyVariant;
pub use dress::HeadItems;
pub use expressions::Expressions;
pub use forge_items::ForgeItems;
pub use head::{Head, HeadError};
pub use loadout::{Replaced, is_worn_type};
pub use look::Look;
pub use npc::NpcHead;
pub use piece::Piece;
pub use shape::MAX_DNA_RIG_LEN;
pub use wearable::Wearable;
pub use zones::ZoneChunk;
pub(crate) use zones::{ZoneList, covered, read_zones, uncovered};

use crate::Issue;

/// Where a part of an [`Armour`] comes from.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PartOrigin {
    /// The skin of the default body.
    Body,
    /// The skin of a [`Piece`].
    Piece,
}

/// One skin worn on the skeleton of an [`Armour`].
#[derive(Debug, Clone)]
pub struct ArmourPart {
    pub(crate) name: String,
    pub(crate) origin: PartOrigin,
    pub(crate) file: String,
    pub(crate) model: Arc<Model>,
    pub(crate) materials: Arc<MaterialStatus>,
    pub(crate) morphs: Vec<MorphTarget>,
    pub(crate) follows: Vec<usize>,
    pub(crate) face: Option<Arc<facial::FaceLink>>,
    pub(crate) mount: Option<RigMount>,
}

impl ArmourPart {
    pub(crate) fn rig_piece(&self) -> RigPiece<'_> {
        RigPiece::new(&self.name, &self.model, &self.materials)
            .with_morphs(&self.morphs)
            .mounted(self.mount)
    }

    /// Returns the name of the part's node: `body` for the skin of the body, else the name of
    /// the piece.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns where the part comes from.
    #[must_use]
    pub const fn origin(&self) -> PartOrigin {
        self.origin
    }

    /// Returns the path of the model file as it was named, a path below `Data`.
    #[must_use]
    pub fn file(&self) -> &str {
        &self.file
    }

    /// Returns the part's model.
    #[must_use]
    pub fn model(&self) -> &Model {
        &self.model
    }

    /// Returns the morph targets of the part's mesh: the expression blendshapes the part was built
    /// with, see [`Piece::with_expressions`].
    #[must_use]
    pub fn morphs(&self) -> &[MorphTarget] {
        &self.morphs
    }

    /// Returns the material library the part is drawn with.
    #[must_use]
    pub fn materials(&self) -> &MaterialStatus {
        &self.materials
    }
}

/// A body of one variant with its skeleton, wearing pieces of armour and clothing, as a set of
/// skins on one skeleton.
///
/// Built by [`Armour::build`]; [`Armour::write_glb`] exports it as one skinned GLB.
#[derive(Debug, Clone)]
pub struct Armour {
    pub(crate) variant: BodyVariant,
    pub(crate) skeleton: Arc<Model>,
    pub(crate) skeleton_file: String,
    pub(crate) parts: Vec<ArmourPart>,
    pub(crate) issues: Vec<Issue>,
    pub(crate) lod: u32,
}

impl Armour {
    /// Returns the body variant.
    #[must_use]
    pub const fn variant(&self) -> BodyVariant {
        self.variant
    }

    /// Returns the model that holds the skeleton of the body, a file without meshes.
    #[must_use]
    pub fn skeleton(&self) -> &Model {
        &self.skeleton
    }

    /// Returns the path of the skeleton file, a path below `Data`.
    #[must_use]
    pub fn skeleton_file(&self) -> &str {
        &self.skeleton_file
    }

    /// Returns the parts: the skins of the body first, then those of the pieces in order.
    #[must_use]
    pub fn parts(&self) -> &[ArmourPart] {
        &self.parts
    }

    /// Returns the number of parts of `origin`.
    #[must_use]
    pub fn count(&self, origin: PartOrigin) -> usize {
        self.parts
            .iter()
            .filter(|part| part.origin == origin)
            .count()
    }

    /// Returns what could not be resolved or done, in the order of the pieces: pieces that cannot
    /// be fitted to the variant are among them, see [`IssueKind::skips`](crate::IssueKind::skips).
    #[must_use]
    pub fn issues(&self) -> &[Issue] {
        &self.issues
    }

    /// Returns, per part, the layered materials that have no diffuse map, neither their own nor
    /// one in a layer, and are drawn in the colours of their layers; report them when the
    /// textures are drawn.
    #[must_use]
    pub fn layer_issues(&self) -> Vec<Issue> {
        self.parts
            .iter()
            .filter_map(flat_layers::flat_layers)
            .collect()
    }

    /// Returns the level of detail the models were read at.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Writes the Armour as one GLB file to `out`: the bones of the skeleton as nodes, one skin
    /// over them, and a skinned mesh node per part named after it; `out` should be buffered.
    /// `options.lod` should be the level the Armour was built at.
    ///
    /// # Errors
    ///
    /// Returns [`GltfError::TooLarge`] when the Armour does not fit in a GLB file and
    /// [`GltfError::Write`] when `out` fails.
    pub fn write_glb(
        &self,
        options: &GlbOptions,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let pieces: Vec<RigPiece<'_>> = self.parts.iter().map(ArmourPart::rig_piece).collect();
        write_rigged_glb(&self.skeleton.bones, &pieces, options, out)
    }

    /// Writes the Armour as one GLB file to `out` like [`Self::write_glb`], taking the textures
    /// of [`MaterialMode::Textures`](p4kx_gltf::MaterialMode::Textures) from `textures`.
    ///
    /// # Errors
    ///
    /// As [`Self::write_glb`].
    pub fn write_glb_with(
        &self,
        options: &GlbOptions,
        textures: &dyn TextureLoader,
        out: &mut impl Write,
    ) -> Result<GlbSummary, GltfError> {
        let pieces: Vec<RigPiece<'_>> = self.parts.iter().map(ArmourPart::rig_piece).collect();
        write_rigged_glb_with(&self.skeleton.bones, &pieces, options, textures, out)
    }
}
