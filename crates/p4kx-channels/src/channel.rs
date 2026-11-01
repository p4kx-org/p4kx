// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::path::{Path, PathBuf};

use p4kx_archive::Build;

use crate::{ChannelKind, PublicPatches};

const UNKNOWN: &str = "unknown";

/// Where a Channel was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChannelSource {
    /// Below the default Library folder, also the one inside a Wine prefix.
    DefaultPath,
    /// Below a Library folder named in the RSI Launcher log.
    Launcher,
    /// In a manually added folder or as a manually added Archive.
    Manual,
}

impl fmt::Display for ChannelSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::DefaultPath => "default",
            Self::Launcher => "launcher",
            Self::Manual => "manual",
        })
    }
}

/// One detected Channel: its Archive and, when the manifest is readable, its Build.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Channel {
    name: String,
    archive: PathBuf,
    build: Option<Build>,
    source: ChannelSource,
}

impl Channel {
    pub(crate) fn new(archive: PathBuf, source: ChannelSource, patches: &PublicPatches) -> Self {
        let name = channel_name(&archive).unwrap_or_else(|| UNKNOWN.to_owned());
        let build = archive
            .parent()
            .and_then(|folder| Build::read_from(folder).ok().flatten())
            .map(|build| {
                let patch = patches.of(&build).map(str::to_owned);
                build.with_public_patch(patch)
            });
        Self {
            name,
            archive,
            build,
            source,
        }
    }

    /// Returns the Channel name, the name of the folder holding the Archive.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the kind of the Channel, told by its name.
    #[must_use]
    pub fn kind(&self) -> ChannelKind {
        ChannelKind::from_name(&self.name)
    }

    /// Returns the path of the Archive.
    #[must_use]
    pub fn archive(&self) -> &Path {
        &self.archive
    }

    /// Returns the Build, or `None` when it is unknown.
    #[must_use]
    pub const fn build(&self) -> Option<&Build> {
        self.build.as_ref()
    }

    /// Returns where the Channel was found.
    #[must_use]
    pub const fn source(&self) -> ChannelSource {
        self.source
    }
}

/// Returns the Channel name of the Archive at `archive`: the name of the folder holding it.
#[must_use]
pub fn channel_name(archive: &Path) -> Option<String> {
    let absolute = std::path::absolute(archive).ok()?;
    let name = absolute.parent()?.file_name()?;
    Some(name.to_string_lossy().into_owned())
}
