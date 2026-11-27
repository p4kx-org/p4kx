// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_extraction::{EntryOrigin, ModelArchiveSource};

/// The physical read of a rig file used to shape or animate a character.
#[derive(Debug, Clone)]
pub struct RigArchiveRead {
    /// The rig request passed to the model provider.
    pub path: String,
    /// The physical Archive input, when the provider can prove it.
    pub entry: Option<EntryOrigin>,
}

/// The original level-zero inputs used to carry expression targets to a lower model level.
#[derive(Debug, Clone)]
pub struct LowerMorphSources {
    /// The actual level-zero model and geometry sources.
    pub model: Arc<[ModelArchiveSource]>,
    /// The level-zero DNA rig.
    pub rig: RigArchiveRead,
}

/// The model and rig inputs supplying the shaped geometry of a part.
#[derive(Debug, Clone)]
pub struct ShapeArchiveSources {
    /// The model supplying the surface on which shaping operates.
    pub model: Arc<[ModelArchiveSource]>,
    /// The DNA rig used to shape the source model.
    pub rig: RigArchiveRead,
    /// The additional sources when expression targets come from level zero.
    pub lower_morphs: Option<LowerMorphSources>,
}
