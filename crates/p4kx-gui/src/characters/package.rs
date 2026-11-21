// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    AnimationFolders, ArchiveModels, CharacterAnimations, LoadoutChanges, character_animations,
};
use p4kx_blender::{CharacterScene, PackageWriteContext, write_character_package_with_context};
use p4kx_extraction::{
    AnimationSet, ArchiveWriteProtection, CancelToken, ConversionError, GlbSummary,
};

use super::export::{CharacterExportSummary, Ended, Request};
use crate::assembly_export::{PartIssue, StoppableTextures, failure_of, info_of, package_failure};
use crate::{EntryIndex, ModelExportFailure, NoEntries};

/// Reads the animations of the animation folders of the Character: the folder of the story
/// character of its Loadout first, then those of its body.
fn clips(models: &ArchiveModels<'_>, request: &Request) -> Result<AnimationSet, ConversionError> {
    let variant = request.armour.variant();
    let folders = request
        .origin
        .npc_folder
        .as_deref()
        .map_or_else(AnimationFolders::default, |folder| {
            AnimationFolders::of_npc(models, variant, folder)
        });
    match character_animations(models, variant, (&["all".to_owned()], &[]), &folders) {
        Err(ConversionError::UnknownAnimations { .. }) => Ok(AnimationSet::default()),
        found => found,
    }
}

/// Writes the Character of `request` as a Blender package, replacing an earlier package of the
/// same folder only once the new one is complete.
///
/// The package carries the skeleton and the shape keys of the Character as built and, when the
/// request asks for them, the animations of its animation folders, read here so that a Cancel
/// stops before the package is written.
pub(super) fn write(index: &EntryIndex, request: &Request, cancel: &CancelToken) -> Ended {
    let Ok(archive) = index.entries_archive() else {
        return NoEntries.into();
    };
    let models = ArchiveModels::new(archive);
    let found = if request.animations {
        match clips(&models, request) {
            Ok(found) => found,
            Err(_) if cancel.is_cancelled() => return Ended::Cancelled,
            Err(error) => return Ended::Failed(failure_of(index, &error)),
        }
    } else {
        AnimationSet::default()
    };
    if cancel.is_cancelled() {
        return Ended::Cancelled;
    }
    let animations = if request.animations {
        request.armour.animate(&found.animations, &models)
    } else {
        CharacterAnimations::default()
    };
    let archive_textures = models.textures();
    let textures = StoppableTextures {
        textures: &archive_textures,
        cancel,
    };
    let scene = CharacterScene::new(&request.name, &request.armour, &animations).with_origin(
        request.origin.chf.as_deref(),
        request.origin.loadout.as_deref(),
    );
    let info = info_of(index, &LoadoutChanges::default());
    let protection = match ArchiveWriteProtection::new(index.path(), cancel.flag()) {
        Ok(protection) => protection,
        Err(error) if error.is_cancelled() => return Ended::Cancelled,
        Err(error) => return Ended::Failed(ModelExportFailure::Refused(error)),
    };
    let context = PackageWriteContext::new(&protection, cancel.flag());
    let written = write_character_package_with_context(
        &scene,
        &textures,
        &request.options(),
        &info,
        &request.file,
        &context,
    );
    match written {
        Ok(package) => Ended::Written(Box::new(CharacterExportSummary {
            file: request.file.clone(),
            name: request.name.clone(),
            parts: request.armour.parts().len(),
            lod: request.armour.lod(),
            written: GlbSummary::default(),
            issues: package.issues.iter().map(PartIssue::of_package).collect(),
            package: Some(package),
        })),
        Err(error) => package_failure(index, error, cancel.is_cancelled())
            .map_or(Ended::Cancelled, Ended::Failed),
    }
}
