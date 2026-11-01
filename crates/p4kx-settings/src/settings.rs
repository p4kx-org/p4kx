// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::profile::profile_folder;
use crate::{
    AssemblyExportSettings, ModelExportSettings, RecentArchives, SavedQueries, UiRenderSettings,
};

const FILE_NAME: &str = "settings.toml";

/// The largest settings file read, in bytes; a larger one is an error.
pub const MAX_SETTINGS_SIZE: u64 = 16 * 1024 * 1024;

/// The persistent settings of P4KX.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The interface language as a BCP 47 tag such as `de-DE`, or `None` to follow the system.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    /// The language Record previews show localisation keys in, as a language code such as `de`,
    /// or `none` for the keys alone, or `None` to follow the interface language.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub record_language: Option<String>,
    /// The folder last chosen for an Extraction, or `None` before the first one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub extraction_folder: Option<PathBuf>,
    /// Whether an Extraction converts `CryXml` Entries and textures; off until the user switches
    /// it on.
    pub extraction_conversion: bool,
    /// What a Conversion writes for a sound and for each WEM a soundbank embeds.
    pub extraction_audio: ExtractionAudio,
    /// What a Conversion writes for a texture.
    pub extraction_textures: ExtractionTextures,
    /// Whether the GUI asks GitHub for a newer Release at start; off until the user switches it on.
    pub update_check: bool,
    /// The recently opened Archives.
    #[serde(skip_serializing_if = "RecentArchives::is_empty")]
    pub recent: RecentArchives,
    /// The manually added Channel sources.
    pub channels: ManualChannels,
    /// The last position and size of the main window, or `None` for the default placement.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub window: Option<WindowGeometry>,
    /// The saved Queries.
    #[serde(skip_serializing_if = "SavedQueries::is_empty")]
    pub queries: SavedQueries,
    /// The choices of the Export as GLB dialog.
    pub model_export: ModelExportSettings,
    /// The choices of the Export of an Entity as GLB.
    pub assembly_export: AssemblyExportSettings,
    /// The choices of the UI tab.
    pub ui: UiRenderSettings,
}

/// What a Conversion writes for a DDS texture; the outputs combine.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ExtractionTextures {
    /// Whether the texture is merged into one DDS file.
    pub merge: bool,
    /// Whether one mip is exported as PNG.
    pub png: bool,
    /// The mip level the PNG holds, 0 being the largest.
    pub mip: u32,
    /// Whether the raw Entries are written as well.
    pub keep_raw: bool,
}

impl Default for ExtractionTextures {
    /// Merges the texture and exports its largest mip as PNG, without the raw Entries.
    fn default() -> Self {
        Self {
            merge: true,
            png: true,
            mip: 0,
            keep_raw: false,
        }
    }
}

/// What a Conversion writes for a WEM sound and for each WEM a soundbank embeds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExtractionAudio {
    /// Ogg where the codec has an Ogg form, WAV otherwise.
    #[default]
    Ogg,
    /// WAV.
    Wav,
    /// The WEM unchanged.
    Raw,
}

/// Position and size of the main window in logical points.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct WindowGeometry {
    /// Left edge of the outer window.
    pub x: i32,
    /// Top edge of the outer window.
    pub y: i32,
    /// Width of the window content.
    pub width: u32,
    /// Height of the window content.
    pub height: u32,
    /// Whether the window is maximised.
    #[serde(default)]
    pub maximized: bool,
}

/// Manually added folders and Archives, searched in addition to the detected Channels.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct ManualChannels {
    /// Channel folders or library folders.
    pub folders: Vec<PathBuf>,
    /// Archive files.
    pub archives: Vec<PathBuf>,
}

/// A failure to read or write the settings file.
#[derive(Debug, Error)]
pub enum SettingsError {
    /// The file or its folder could not be read or written.
    #[error("cannot access the settings file {path}")]
    Io {
        /// Path of the settings file.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The file is not valid settings TOML.
    #[error("the settings file {path} is malformed")]
    Malformed {
        /// Path of the settings file.
        path: PathBuf,
        /// What the TOML parser reported.
        #[source]
        source: toml::de::Error,
    },
    /// The file is, or would be written, larger than [`MAX_SETTINGS_SIZE`].
    #[error("the settings file {path} is larger than {MAX_SETTINGS_SIZE} bytes")]
    TooLarge {
        /// Path of the settings file.
        path: PathBuf,
    },
    /// The settings could not be written as TOML.
    #[error("cannot write the settings as TOML")]
    Serialize(#[from] toml::ser::Error),
}

/// Returns `settings.toml` in the `P4KX` folder of the platform config directory,
/// `%APPDATA%\P4KX` on Windows and `$XDG_CONFIG_HOME/P4KX` on Linux.
///
/// The first call migrates the old `P4K-Extractor` folder (see
/// [`migrate_profile`](crate::migrate_profile)).
#[must_use]
pub fn default_settings_file() -> Option<PathBuf> {
    profile_folder().map(|dir| dir.join(FILE_NAME))
}

impl Settings {
    /// Reads the settings from `file`, or returns the defaults when it does not exist.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the file cannot be read, is larger than
    /// [`MAX_SETTINGS_SIZE`] or is malformed.
    pub fn load(file: &Path) -> Result<Self, SettingsError> {
        let opened = match std::fs::File::open(file) {
            Ok(opened) => opened,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => return Err(io_error(file, source)),
        };
        let mut text = String::new();
        opened
            .take(MAX_SETTINGS_SIZE + 1)
            .read_to_string(&mut text)
            .map_err(|source| io_error(file, source))?;
        if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_SETTINGS_SIZE {
            return Err(SettingsError::TooLarge {
                path: file.to_path_buf(),
            });
        }
        toml::from_str(&text).map_err(|source| SettingsError::Malformed {
            path: file.to_path_buf(),
            source,
        })
    }

    /// Writes the settings to `file`, creating its folder when missing. The text goes to a
    /// temporary file next to it first, which then replaces `file`, so a crash never leaves a
    /// half-written file.
    ///
    /// # Errors
    ///
    /// Returns a [`SettingsError`] when the text would be larger than [`MAX_SETTINGS_SIZE`],
    /// so that it could not be read again, or when the folder or the file cannot be written.
    pub fn save(&self, file: &Path) -> Result<(), SettingsError> {
        let text = toml::to_string(self)?;
        if u64::try_from(text.len()).unwrap_or(u64::MAX) > MAX_SETTINGS_SIZE {
            return Err(SettingsError::TooLarge {
                path: file.to_path_buf(),
            });
        }
        if let Some(folder) = file.parent() {
            std::fs::create_dir_all(folder).map_err(|source| io_error(file, source))?;
        }
        let mut temporary = file.as_os_str().to_owned();
        temporary.push(format!(".{}.tmp", std::process::id()));
        let temporary = PathBuf::from(temporary);
        let written = write_synced(&temporary, text.as_bytes())
            .and_then(|()| std::fs::rename(&temporary, file));
        if let Err(source) = written {
            let _ignored = std::fs::remove_file(&temporary);
            return Err(io_error(file, source));
        }
        Ok(())
    }
}

impl ManualChannels {
    /// Adds `folder` unless it is already listed; returns whether it was added.
    pub fn add_folder(&mut self, folder: PathBuf) -> bool {
        add(&mut self.folders, folder)
    }

    /// Adds `archive` unless it is already listed; returns whether it was added.
    pub fn add_archive(&mut self, archive: PathBuf) -> bool {
        add(&mut self.archives, archive)
    }

    /// Adds every folder and Archive of `other` that is not listed yet.
    pub fn merge(&mut self, other: &Self) {
        for folder in &other.folders {
            self.add_folder(folder.clone());
        }
        for archive in &other.archives {
            self.add_archive(archive.clone());
        }
    }

    /// Removes `path` from the folders and the Archives, matching equal or equivalent paths;
    /// returns whether it was listed.
    pub fn remove(&mut self, path: &Path) -> bool {
        let before = self.folders.len() + self.archives.len();
        self.folders.retain(|folder| !same_path(folder, path));
        self.archives.retain(|archive| !same_path(archive, path));
        before != self.folders.len() + self.archives.len()
    }
}

fn add(paths: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    if paths.contains(&path) {
        return false;
    }
    paths.push(path);
    true
}

fn same_path(stored: &Path, path: &Path) -> bool {
    stored == path
        || matches!(
            (std::fs::canonicalize(stored), std::fs::canonicalize(path)),
            (Ok(a), Ok(b)) if a == b
        )
}

fn write_synced(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let mut file = std::fs::File::create(path)?;
    file.write_all(bytes)?;
    file.sync_all()
}

fn io_error(path: &Path, source: io::Error) -> SettingsError {
    SettingsError::Io {
        path: path.to_path_buf(),
        source,
    }
}
