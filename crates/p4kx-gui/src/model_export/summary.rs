// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use p4kx_extraction::model::{MaterialStatus, Model};
use p4kx_extraction::{AnimationSet, GlbSummary, Guid, TextureRole, WriteProtectionError};

use crate::model_info::{MAX_LISTED_ANIMATIONS, printable};
use crate::{ChunkNote, ModelInfo, NoEntries};

/// What a finished Export as GLB wrote and what it left out.
#[derive(Debug, Clone, PartialEq)]
pub struct ModelExportSummary {
    file: PathBuf,
    name: String,
    lod: u32,
    written: GlbSummary,
    unread: Vec<ChunkNote>,
    unread_total: usize,
    missing_textures: Vec<MissingTexture>,
    material_problem: Option<MaterialProblem>,
    unnamed_sub_materials: usize,
    missing_layers: usize,
    animations_asked: bool,
    unread_animations: Vec<AnimationNote>,
    unread_animations_total: usize,
}

/// An animation file or an animation that was not written, with the reason.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnimationNote {
    /// The Entry path of the animation file.
    pub source: String,
    /// The animation's name when only this animation of a database was left out, and always for
    /// an animation no bone matched.
    pub name: Option<String>,
    /// Why it was left out.
    pub kind: AnimationNoteKind,
}

/// Why an animation is missing from the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnimationNoteKind {
    /// The file or animation could not be read, or is additive, for the reason given.
    Unread {
        /// Why it was left out.
        reason: String,
    },
    /// The animation was read, but none of its tracks matches a bone of the model, so it has no
    /// channel in the file.
    NoBoneMatched {
        /// The number of tracks that were left out.
        skipped_tracks: usize,
    },
}

/// Why the material library of a model was not used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MaterialProblem {
    /// The library as the model names it, below `Data` without `.mtl`.
    pub library: String,
    /// Why it cannot be read, or `None` when it does not exist.
    pub reason: Option<String>,
}

/// A texture of a material that the file lacks, so the material keeps its plain colour.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MissingTexture {
    /// The material the texture belongs to.
    pub material: String,
    /// What the texture would have been used for.
    pub role: TextureRole,
    /// The texture file as the material names it.
    pub file: String,
    /// Why it is not used.
    pub reason: String,
}

impl ModelExportSummary {
    pub(crate) fn new(
        file: PathBuf,
        lod: u32,
        model: &Model,
        (written, animations): (GlbSummary, &AnimationSet),
        animations_asked: bool,
    ) -> Self {
        let info = ModelInfo::of(model);
        let unmatched = unmatched(&written, animations);
        Self {
            file,
            name: info.name,
            lod,
            unread_total: info.chunks.skipped + info.chunks.unknown,
            missing_textures: written
                .texture_fallbacks
                .iter()
                .map(|fallback| MissingTexture {
                    material: printable(&fallback.material),
                    role: fallback.role,
                    file: printable(&fallback.file),
                    reason: printable(&fallback.reason),
                })
                .collect(),
            written,
            unread: info.unread,
            material_problem: match &model.materials {
                MaterialStatus::Missing { path } => Some(MaterialProblem {
                    library: printable(path),
                    reason: None,
                }),
                MaterialStatus::Unreadable { path, reason } => Some(MaterialProblem {
                    library: printable(path),
                    reason: Some(printable(reason)),
                }),
                _ => None,
            },
            unnamed_sub_materials: model
                .materials
                .library()
                .map_or(0, |library| library.unknown_entries),
            missing_layers: model
                .materials
                .library()
                .map_or(0, |library| library.unresolved_layers.len()),
            animations_asked,
            unread_animations_total: animations.unread.len() + unmatched.len(),
            unread_animations: animations
                .unread
                .iter()
                .map(|unread| AnimationNote {
                    source: printable(&unread.source),
                    name: unread.name.as_deref().map(printable),
                    kind: AnimationNoteKind::Unread {
                        reason: printable(&unread.reason),
                    },
                })
                .chain(unmatched)
                .take(MAX_LISTED_ANIMATIONS)
                .collect(),
        }
    }

    /// Returns whether the export was asked to write the animations.
    #[must_use]
    pub const fn animations_asked(&self) -> bool {
        self.animations_asked
    }

    /// Returns how many animations the file holds.
    #[must_use]
    pub fn animations_written(&self) -> usize {
        self.written
            .animations
            .iter()
            .filter(|animation| animation.channels > 0)
            .count()
    }

    /// Returns the animation files and animations that were not written because they could not
    /// be read, are additive or match no bone, at most [`MAX_LISTED_ANIMATIONS`].
    #[must_use]
    pub fn unread_animations(&self) -> &[AnimationNote] {
        &self.unread_animations
    }

    /// Returns how many animation files and animations were not written, of which
    /// [`Self::unread_animations`] lists the first ones.
    #[must_use]
    pub const fn unread_animations_total(&self) -> usize {
        self.unread_animations_total
    }

    /// Returns the GLB file that was written.
    #[must_use]
    pub fn file(&self) -> &Path {
        &self.file
    }

    /// Returns the file name of the model.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the level of detail that was exported.
    #[must_use]
    pub const fn lod(&self) -> u32 {
        self.lod
    }

    /// Returns what the writer counted: nodes, meshes, primitives, materials, vertices, triangles,
    /// bones and the length of the file.
    #[must_use]
    pub const fn written(&self) -> &GlbSummary {
        &self.written
    }

    /// Returns the chunks of the model's files that were skipped or are of an unknown type.
    #[must_use]
    pub fn unread(&self) -> &[ChunkNote] {
        &self.unread
    }

    /// Returns how many chunks were skipped or are of an unknown type, of which [`Self::unread`]
    /// lists the first ones.
    #[must_use]
    pub const fn unread_total(&self) -> usize {
        self.unread_total
    }

    /// Returns the textures the file lacks; empty unless the materials were written with
    /// textures.
    #[must_use]
    pub fn missing_textures(&self) -> &[MissingTexture] {
        &self.missing_textures
    }

    /// Returns what went wrong with the material library, when the model names one that is
    /// missing or unreadable; the export used the default material then.
    #[must_use]
    pub const fn material_problem(&self) -> Option<&MaterialProblem> {
        self.material_problem.as_ref()
    }

    /// Returns how many entries of the material library are no material and read as the default
    /// material.
    #[must_use]
    pub const fn unnamed_sub_materials(&self) -> usize {
        self.unnamed_sub_materials
    }

    /// Returns how many layer files of the material library are missing or unreadable; their
    /// materials keep the colours of their own file.
    #[must_use]
    pub const fn missing_layers(&self) -> usize {
        self.missing_layers
    }
}

/// Returns a note for each animation the file holds without a channel, because no track of it
/// matches a bone of the model.
fn unmatched(written: &GlbSummary, animations: &AnimationSet) -> Vec<AnimationNote> {
    written
        .animations
        .iter()
        .zip(&animations.origins)
        .filter(|(animation, _)| animation.channels == 0)
        .map(|(animation, origin)| AnimationNote {
            source: printable(origin),
            name: Some(printable(&animation.name)),
            kind: AnimationNoteKind::NoBoneMatched {
                skipped_tracks: animation.skipped_tracks.len(),
            },
        })
        .collect()
}

/// Why an Export as GLB wrote no file.
#[derive(Debug, Clone)]
pub enum ModelExportFailure {
    /// The model is encrypted and the Archive has no key.
    KeyRequired,
    /// The model is encrypted and the Archive key does not decrypt it.
    KeyRejected,
    /// The Archive changed on disk, so the model was not exported.
    ArchiveChanged,
    /// The output violates a protection or identity boundary.
    Refused(WriteProtectionError),
    /// The DataForge has no Record with this GUID.
    NoRecord(Guid),
    /// The model could not be read or the file not written, for the reason given.
    Failed(String),
    /// The export stopped without a result.
    Stopped,
}

impl From<NoEntries> for ModelExportFailure {
    fn from(error: NoEntries) -> Self {
        Self::Failed(error.to_string())
    }
}
