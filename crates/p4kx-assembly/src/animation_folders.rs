// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{AnimationChoice, AnimationSet, ArchiveAnimationRead, ConversionError};

use crate::{ArchiveModels, BodyVariant};

/// The folders the animations of a Character are looked up in before the folders of its body, and
/// whether the lookup falls back to those.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct AnimationFolders {
    folders: Vec<String>,
    fallback: bool,
}

impl AnimationFolders {
    /// Looks only in `folders`; none means the folders of the body.
    #[must_use]
    pub fn only(folders: &[String]) -> Self {
        Self {
            folders: folders.to_vec(),
            fallback: false,
        }
    }

    /// Looks in the folder of the story character's own animations first,
    /// `Animations/pu/<body>/<name>`, where `name` is the name of the head's own folder `folder` up
    /// to its first underscore; a name or path that folder lacks is looked for in the folders of
    /// the body. Only the folders of the body when the Archive has no such folder.
    #[must_use]
    pub fn of_npc(models: &ArchiveModels<'_>, variant: BodyVariant, folder: &str) -> Self {
        let name = folder
            .rsplit(['/', '\\'])
            .next()
            .and_then(|last| last.split('_').next())
            .filter(|name| !name.is_empty());
        let own = name
            .map(|name| format!("Animations/pu/{}/{name}", variant.code()))
            .filter(|own| models.has_folder(own));
        Self {
            folders: own.into_iter().collect(),
            fallback: true,
        }
    }

    /// Returns the folder looked in first, if there is one and the folders were not named.
    #[must_use]
    pub fn own(&self) -> Option<&str> {
        self.folders
            .first()
            .map(String::as_str)
            .filter(|_| self.fallback)
    }
}

/// Returns the choice `names` stand for: none for no name, all when a name is `all`, else the
/// named animations.
#[must_use]
pub fn animation_choice(names: &[String]) -> Option<AnimationChoice> {
    if names.is_empty() {
        None
    } else if names.iter().any(|name| name.eq_ignore_ascii_case("all")) {
        Some(AnimationChoice::All)
    } else {
        Some(AnimationChoice::Named(names.to_vec()))
    }
}

/// Reads the animations `names` and the extra sources `sources` name for a Character of `variant`
/// from the folders `folders` choose, else those of its body; none without a name.
///
/// # Errors
///
/// As [`ArchiveModels::animations`].
pub fn character_animations(
    models: &ArchiveModels<'_>,
    variant: BodyVariant,
    requested: (&[String], &[String]),
    folders: &AnimationFolders,
) -> Result<AnimationSet, ConversionError> {
    character_animations_with_archive_sources(models, variant, requested, folders)
        .map(ArchiveAnimationRead::into_set)
}

/// Selects Character animations with the bound Archive Entry for each successful read.
///
/// # Errors
/// As [`character_animations`].
pub fn character_animations_with_archive_sources(
    models: &ArchiveModels<'_>,
    variant: BodyVariant,
    (names, sources): (&[String], &[String]),
    folders: &AnimationFolders,
) -> Result<ArchiveAnimationRead, ConversionError> {
    let Some(choice) = animation_choice(names) else {
        return Ok(ArchiveAnimationRead::default());
    };
    if !folders.folders.is_empty() {
        let own: Vec<&str> = folders.folders.iter().map(String::as_str).collect();
        match models.animations_with_archive_sources(&own, &choice, sources) {
            Ok(found) => return Ok(found),
            Err(ConversionError::UnknownAnimations { .. }) if folders.fallback => {}
            Err(error) => return Err(error),
        }
    }
    models.animations_with_archive_sources(&variant.animation_folders(), &choice, sources)
}
