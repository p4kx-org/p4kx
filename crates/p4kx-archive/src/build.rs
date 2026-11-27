// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use thiserror::Error;

use crate::BuildManifestObservation;
use crate::build_manifest_origin::MANIFEST_NAME;

/// The Build of an Archive as stated in its Channel's `build_manifest.id`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Build {
    branch: String,
    version: String,
    change_number: String,
    id: String,
    public_patch: Option<String>,
}

/// A `build_manifest.id` that exists but cannot be read.
#[derive(Debug, Error)]
pub enum BuildError {
    /// The manifest could not be read.
    #[error("cannot read {path}")]
    Io {
        /// Path of the manifest.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: io::Error,
    },
    /// The manifest is not the JSON the launcher writes.
    #[error("{path} is not a valid build manifest")]
    Malformed {
        /// Path of the manifest.
        path: PathBuf,
        /// What the JSON parser reported.
        #[source]
        source: serde_json::Error,
    },
}

#[derive(Deserialize)]
struct Manifest {
    #[serde(rename = "Data")]
    data: ManifestData,
}

#[derive(Deserialize)]
#[serde(rename_all = "PascalCase")]
struct ManifestData {
    branch: String,
    version: String,
    #[serde(rename = "RequestedP4ChangeNum")]
    change_number: String,
    build_id: String,
}

impl Build {
    /// Reads the Build from the `build_manifest.id` in `channel_folder`, or returns `None` when
    /// the folder has no manifest.
    ///
    /// # Errors
    ///
    /// Returns a [`BuildError`] when the manifest exists but cannot be read or parsed.
    pub fn read_from(channel_folder: impl AsRef<Path>) -> Result<Option<Self>, BuildError> {
        read_manifest(channel_folder.as_ref()).into_result()
    }

    /// Returns the Build with `public_patch` as its public patch, replacing any earlier one.
    ///
    /// Nothing is read: the caller takes the value from the RSI Launcher log.
    #[must_use]
    pub fn with_public_patch(mut self, public_patch: Option<String>) -> Self {
        self.public_patch = public_patch;
        self
    }

    /// Returns the public patch version, e.g. `4.7.2`, or `None` when none was set.
    #[must_use]
    pub fn public_patch(&self) -> Option<&str> {
        self.public_patch.as_deref()
    }

    /// Returns the branch, e.g. `sc-alpha-4.10.2`.
    #[must_use]
    pub fn branch(&self) -> &str {
        &self.branch
    }

    /// Returns the version, e.g. `4.10.197.2878`.
    #[must_use]
    pub fn version(&self) -> &str {
        &self.version
    }

    /// Returns the Perforce change number the Build was made from.
    #[must_use]
    pub fn change_number(&self) -> &str {
        &self.change_number
    }

    /// Returns the `BuildId` GUID.
    #[must_use]
    pub fn build_id(&self) -> &str {
        &self.id
    }
}

pub(crate) struct ObservedBuild {
    pub(crate) build: Option<Build>,
    pub(crate) observation: BuildManifestObservation,
}

impl ObservedBuild {
    pub(crate) const fn missing() -> Self {
        Self {
            build: None,
            observation: BuildManifestObservation::missing(),
        }
    }
}

enum ManifestRead {
    Missing,
    Unreadable(BuildError),
    Parsed { build: Build, byte_length: u64 },
    Malformed { error: BuildError, byte_length: u64 },
}

impl ManifestRead {
    fn into_result(self) -> Result<Option<Build>, BuildError> {
        match self {
            Self::Missing => Ok(None),
            Self::Unreadable(error) | Self::Malformed { error, .. } => Err(error),
            Self::Parsed { build, .. } => Ok(Some(build)),
        }
    }

    fn into_observed(self) -> ObservedBuild {
        match self {
            Self::Missing => ObservedBuild::missing(),
            Self::Unreadable(_) => ObservedBuild {
                build: None,
                observation: BuildManifestObservation::unreadable(),
            },
            Self::Parsed { build, byte_length } => ObservedBuild {
                build: Some(build),
                observation: BuildManifestObservation::parsed(byte_length),
            },
            Self::Malformed { byte_length, .. } => ObservedBuild {
                build: None,
                observation: BuildManifestObservation::malformed(byte_length),
            },
        }
    }
}

pub(crate) fn read_observed(channel_folder: &Path) -> ObservedBuild {
    read_manifest(channel_folder).into_observed()
}

fn read_manifest(channel_folder: &Path) -> ManifestRead {
    let path = channel_folder.join(MANIFEST_NAME);
    let text = match std::fs::read(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return ManifestRead::Missing,
        Err(source) => return ManifestRead::Unreadable(BuildError::Io { path, source }),
    };
    let byte_length = text.len() as u64;
    match serde_json::from_slice::<Manifest>(&text) {
        Ok(manifest) => {
            let data = manifest.data;
            ManifestRead::Parsed {
                build: Build {
                    branch: data.branch,
                    version: data.version,
                    change_number: data.change_number,
                    id: data.build_id,
                    public_patch: None,
                },
                byte_length,
            }
        }
        Err(source) => ManifestRead::Malformed {
            error: BuildError::Malformed { path, source },
            byte_length,
        },
    }
}
