// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Component, Path, PathBuf, Prefix};
use std::sync::{Arc, Mutex, PoisonError};

use p4kx_channels::{Channel, PublicPatches, SearchRecipe};
use p4kx_settings::Settings;

use crate::discovery_notes;
use crate::error::{ToolError, capped};

#[path = "locate_patches.rs"]
mod patches;

use patches::PatchCache;

/// The longest `archive` argument accepted, in bytes.
pub(crate) const MAX_ARCHIVE_LEN: usize = 4096;

/// The kind of file an `archive` argument may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Accepts {
    /// A `.p4k` Archive.
    Archive,
    /// A `.p4k` Archive or a `.dcb` DataForge file, for the DataForge tools.
    Forge,
}

impl Accepts {
    const fn extensions(self) -> &'static [&'static str] {
        match self {
            Self::Archive => &["p4k"],
            Self::Forge => &["p4k", "dcb"],
        }
    }
}

/// Where Channels are looked for: the default Library folder, the launcher folder and the
/// settings file with the manual entries.
#[derive(Debug, Clone)]
pub(crate) struct Locations {
    recipe: SearchRecipe,
    settings_file: Option<PathBuf>,
    patches: Arc<Mutex<PatchCache>>,
}

impl Locations {
    /// Creates the locations; `None` falls back to the platform default.
    pub(crate) fn new(
        default_library: Option<PathBuf>,
        launcher: Option<PathBuf>,
        settings_file: Option<PathBuf>,
    ) -> Self {
        let mut recipe = SearchRecipe::standard();
        if let Some(path) = default_library {
            recipe = recipe.with_default_library(path);
        }
        if let Some(path) = launcher {
            recipe = recipe.with_launcher(path);
        }
        Self::with_recipe(recipe, settings_file)
    }

    /// Captures discovery inputs; manual entries are loaded from the settings file.
    pub(super) fn with_recipe(recipe: SearchRecipe, settings_file: Option<PathBuf>) -> Self {
        Self {
            recipe: recipe.with_manual(Vec::new(), Vec::new()),
            settings_file,
            patches: Arc::new(Mutex::new(PatchCache::default())),
        }
    }

    /// Returns detected Channels and reports incomplete discovery alongside the result.
    pub(crate) fn channels(&self) -> Result<Vec<Channel>, ToolError> {
        let settings = match &self.settings_file {
            Some(file) => Settings::load(file)?,
            None => Settings::default(),
        };
        let snapshot = self
            .recipe
            .snapshot_with_manual(&settings.channels.folders, &settings.channels.archives);
        let report = snapshot.detect();
        discovery_notes::remember(report.issues());
        let complete = report.is_complete();
        let (channels, patches) = report.into_value().into_parts();
        self.patches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .record(&snapshot, patches, complete);
        Ok(channels)
    }

    /// Returns patches from the current source set, reusing only a complete unchanged scan.
    pub(crate) fn public_patches(&self) -> Arc<PublicPatches> {
        let snapshot = self.recipe.snapshot();
        self.patches
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .read(&snapshot)
    }

    /// Returns the Archive path `text` names: a detected Channel by name, ignoring ASCII case,
    /// or the absolute local path of an existing file of the kind `accepts`.
    pub(crate) fn resolve(&self, text: &str, accepts: Accepts) -> Result<PathBuf, ToolError> {
        let text = capped("archive", text, MAX_ARCHIVE_LEN)?;
        if is_channel_name(text) {
            let channels = self.channels()?;
            return channels
                .iter()
                .find(|channel| channel.name().eq_ignore_ascii_case(text))
                .map(|channel| channel.archive().to_path_buf())
                .ok_or_else(|| ToolError::UnknownChannel {
                    name: text.to_owned(),
                    detected: channels
                        .iter()
                        .map(Channel::name)
                        .collect::<Vec<_>>()
                        .join(", "),
                });
        }
        archive_path(text, accepts)
    }
}

fn is_channel_name(text: &str) -> bool {
    !text.is_empty() && !text.contains(['\\', '/', ':', '\0'])
}

fn archive_path(text: &str, accepts: Accepts) -> Result<PathBuf, ToolError> {
    let path = Path::new(text);
    let local = path.is_absolute()
        && !text.contains('\0')
        && path.components().all(|component| match component {
            Component::Prefix(prefix) => matches!(prefix.kind(), Prefix::Disk(_)),
            Component::RootDir => true,
            Component::Normal(name) => !name.as_encoded_bytes().contains(&b':'),
            Component::CurDir | Component::ParentDir => false,
        })
        && has_extension(path, accepts);
    if !local {
        return Err(ToolError::NotAnArchivePath(text.to_owned()));
    }
    if !path.is_file() {
        return Err(ToolError::Missing(path.to_path_buf()));
    }
    if !resolves_locally(path, accepts) {
        return Err(ToolError::NotLocal(path.to_path_buf()));
    }
    Ok(path.to_path_buf())
}

fn has_extension(path: &Path, accepts: Accepts) -> bool {
    path.extension().is_some_and(|extension| {
        accepts
            .extensions()
            .iter()
            .any(|wanted| extension.eq_ignore_ascii_case(wanted))
    })
}

/// Returns whether `path`, with every link followed, is a file of the kind `accepts` on a local
/// drive.
fn resolves_locally(path: &Path, accepts: Accepts) -> bool {
    let Ok(canonical) = std::fs::canonicalize(path) else {
        return false;
    };
    let on_local_drive = !cfg!(windows)
        || matches!(
            canonical.components().next(),
            Some(Component::Prefix(prefix)) if matches!(prefix.kind(), Prefix::VerbatimDisk(_))
        );
    on_local_drive && has_extension(&canonical, accepts)
}

#[cfg(test)]
mod tests {
    use super::{Accepts, ToolError, archive_path, is_channel_name};

    #[test]
    fn a_dcb_file_is_a_path_for_the_dataforge_tools_only() {
        let text = if cfg!(windows) {
            r"C:\p4kx-mcp-missing\Game2.dcb"
        } else {
            "/p4kx-mcp-missing/Game2.dcb"
        };

        assert!(matches!(
            archive_path(text, Accepts::Archive),
            Err(ToolError::NotAnArchivePath(_))
        ));
        assert!(matches!(
            archive_path(text, Accepts::Forge),
            Err(ToolError::Missing(_))
        ));
    }

    #[test]
    fn plain_names_are_channel_names() {
        for name in ["LIVE", "ptu", "TECH-PREVIEW", "4.3"] {
            assert!(is_channel_name(name), "{name} is a Channel name");
        }
        for text in ["", "a/b", r"a\b", "C:", "a\0"] {
            assert!(!is_channel_name(text), "{text:?} is no Channel name");
        }
    }

    #[test]
    fn only_absolute_local_p4k_paths_are_accepted() {
        let rejected = [
            "Data.p4k",
            "./Data.p4k",
            "LIVE/Data.p4k",
            r"\\server\share\Data.p4k",
            r"\\?\C:\Data.p4k",
            r"\\.\C:\Data.p4k",
            "/games/../Data.p4k",
            r"C:\games\..\Data.p4k",
            "/games/Data.zip",
            r"C:\games\Data.zip",
            "/games/Data",
            r"C:\games:stream\Data.p4k",
            r"C:\games\Data.p4k:stream",
            "/games:stream/Data.p4k",
        ];
        for text in rejected {
            assert!(
                matches!(
                    archive_path(text, Accepts::Forge),
                    Err(ToolError::NotAnArchivePath(_))
                ),
                "{text} is rejected"
            );
        }
    }

    #[test]
    fn a_missing_p4k_file_is_named() {
        let text = if cfg!(windows) {
            r"C:\p4kx-mcp-missing\Data.P4K"
        } else {
            "/p4kx-mcp-missing/Data.P4K"
        };
        assert!(
            matches!(
                archive_path(text, Accepts::Archive),
                Err(ToolError::Missing(_))
            ),
            "{text} is missing"
        );
    }
}

#[cfg(test)]
#[path = "locate_source_tests.rs"]
mod source_tests;
