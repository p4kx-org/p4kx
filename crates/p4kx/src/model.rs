// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use anyhow::Result;
use p4kx_extraction::model::{LodSelection, MaterialReading, ModelError, PositionBox};
use p4kx_extraction::{
    AnimationChoice, AnimationListing, AnimationSet, ConversionError, GltfError, ModelSource,
    PathIndex, TextureVerdicts,
};

use crate::cli_model::{ModelAction, ModelArgs, ModelFileSource};
use crate::model_glb;
use crate::model_info::info;

/// Runs `p4kx model info` or `p4kx model glb`.
pub(crate) fn run(args: &ModelArgs, as_json: bool) -> Result<()> {
    match &args.action {
        ModelAction::Info(source) => info(&open_listing(source, as_json)?, as_json),
        action @ ModelAction::Glb { .. } => model_glb::run(action, as_json),
    }
}

/// Reads the animations `choice` selects for the model Entry `entry` of the Archive `index` is
/// over; none without a choice.
pub(crate) fn archive_animations(
    index: &PathIndex<'_>,
    entry: &str,
    source: &ModelFileSource,
    choice: Option<&AnimationChoice>,
) -> Result<AnimationSet> {
    Ok(choice
        .map(|choice| AnimationSet::from_archive(index, entry, choice, &source.animation_sources))
        .transpose()?
        .unwrap_or_default())
}

/// Reads the animations `choice` selects for the model file `source` names; none without a
/// choice.
pub(crate) fn disk_animations(
    source: &ModelFileSource,
    choice: Option<&AnimationChoice>,
) -> Result<AnimationSet> {
    let extra: Vec<PathBuf> = source.animation_sources.iter().map(PathBuf::from).collect();
    Ok(choice
        .map(|choice| AnimationSet::from_disk(&source.path, choice, &extra))
        .transpose()?
        .unwrap_or_default())
}

/// Opens the model `source` names and lists its animations from their headers.
fn open_listing(
    source: &ModelFileSource,
    with_material_receipt: bool,
) -> Result<(ModelSource, AnimationListing)> {
    if let Some(entry) = &source.entry {
        let archive = crate::key::open(&source.path)?;
        let index = PathIndex::new(&archive);
        let verdicts = TextureVerdicts::new();
        let model = if with_material_receipt {
            ModelSource::from_index_with_material_receipt(&index, entry, &verdicts)?
        } else {
            ModelSource::from_index_reading(
                &index,
                entry,
                LodSelection::All,
                PositionBox::default(),
                MaterialReading::Plain,
                &verdicts,
            )?
        };
        let listing = AnimationListing::from_archive(&index, entry, &source.animation_sources)?;
        return Ok((model, listing));
    }
    let model = if with_material_receipt {
        ModelSource::from_file_with_material_receipt(&source.path)?
    } else {
        ModelSource::from_file_reading(&source.path, LodSelection::All, MaterialReading::Plain)?
    };
    let extra: Vec<PathBuf> = source.animation_sources.iter().map(PathBuf::from).collect();
    let listing = AnimationListing::from_disk(&source.path, &extra)?;
    Ok((model, listing))
}

/// Returns whether `error` is a model failure caused by the command line: a geometry companion
/// that does not exist or a LOD the model does not have.
pub(crate) fn is_invalid_input(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ConversionError>(),
        Some(
            ConversionError::Model {
                source: ModelError::MissingGeometry { .. } | ModelError::NoSuchLod { .. },
                ..
            } | ConversionError::UnknownAnimations { .. }
                | ConversionError::NotAnimationFile { .. }
                | ConversionError::Glb {
                    source: GltfError::NoSuchLod { .. },
                    ..
                }
        )
    )
}

/// Returns whether `error` is a model failure caused by an unreadable or corrupt model or
/// companion.
pub(crate) fn is_archive_error(error: &anyhow::Error) -> bool {
    matches!(
        error.downcast_ref::<ConversionError>(),
        Some(
            ConversionError::Model {
                source: ModelError::Companion { .. }
                    | ModelError::CompanionChunks { .. }
                    | ModelError::NoMesh { .. }
                    | ModelError::Corrupt { .. },
                ..
            } | ConversionError::Animation { .. }
        )
    )
}
