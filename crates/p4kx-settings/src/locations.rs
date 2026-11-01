// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The one place that decides where P4KX keeps its profile, caches and data.
//!
//! Windows and macOS use the known folders of the platform; every other Unix follows the XDG Base
//! Directory specification. Callers never branch on the platform for a path.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

/// The application folder inside every base folder.
pub(crate) const APP_FOLDER: &str = "P4KX";

/// Whether this platform has known folders; otherwise the XDG rule applies.
const USE_KNOWN_FOLDERS: bool = cfg!(any(windows, target_os = "macos"));

/// The kind of a base folder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FolderKind {
    /// Settings and the stored Archive key.
    Config,
    /// Data that can be rebuilt.
    Cache,
    /// Data the user keeps.
    Data,
}

/// The environment variables that decide the XDG base folders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct XdgEnvironment {
    home: Option<PathBuf>,
    config_home: Option<OsString>,
    cache_home: Option<OsString>,
    data_home: Option<OsString>,
}

impl XdgEnvironment {
    /// Creates an environment from the home folder and the values of `XDG_CONFIG_HOME`,
    /// `XDG_CACHE_HOME` and `XDG_DATA_HOME`.
    #[must_use]
    pub const fn new(
        home: Option<PathBuf>,
        config_home: Option<OsString>,
        cache_home: Option<OsString>,
        data_home: Option<OsString>,
    ) -> Self {
        Self {
            home,
            config_home,
            cache_home,
            data_home,
        }
    }

    /// Reads the environment of this process.
    #[must_use]
    pub fn from_process() -> Self {
        Self::new(
            dirs::home_dir(),
            std::env::var_os("XDG_CONFIG_HOME"),
            std::env::var_os("XDG_CACHE_HOME"),
            std::env::var_os("XDG_DATA_HOME"),
        )
    }

    /// Returns the base folder of `kind`, `None` without a usable variable or home folder.
    ///
    /// An empty or relative variable is ignored, as the specification requires; the fallback is
    /// `~/.config`, `~/.cache` or `~/.local/share`.
    #[must_use]
    pub fn base(&self, kind: FolderKind) -> Option<PathBuf> {
        let (variable, fallback) = match kind {
            FolderKind::Config => (&self.config_home, ".config"),
            FolderKind::Cache => (&self.cache_home, ".cache"),
            FolderKind::Data => (&self.data_home, ".local/share"),
        };
        let from_variable = variable
            .as_ref()
            .map(PathBuf::from)
            .filter(|path| path.is_absolute());
        from_variable.or_else(|| {
            let home = self.home.as_ref().filter(|home| home.is_absolute())?;
            Some(home.join(fallback))
        })
    }
}

/// Returns the base folder of `kind` on this platform, `None` when it has none.
#[must_use]
pub fn base_folder(kind: FolderKind) -> Option<PathBuf> {
    if USE_KNOWN_FOLDERS {
        match kind {
            FolderKind::Config => dirs::config_dir(),
            FolderKind::Cache => dirs::cache_dir(),
            FolderKind::Data => dirs::data_dir(),
        }
    } else {
        XdgEnvironment::from_process().base(kind)
    }
}

/// Returns the `P4KX` folder for caches, `None` when the platform has no cache folder.
#[must_use]
pub fn cache_folder() -> Option<PathBuf> {
    base_folder(FolderKind::Cache).map(|base| base.join(APP_FOLDER))
}

/// Returns the `P4KX` folder for data the user keeps, `None` when the platform has none.
#[must_use]
pub fn data_folder() -> Option<PathBuf> {
    base_folder(FolderKind::Data).map(|base| base.join(APP_FOLDER))
}

/// Creates `folder` and its missing parents, readable by the user alone (mode 0700) on Unix.
///
/// An existing folder keeps its permissions.
///
/// # Errors
///
/// Returns the I/O error when a folder cannot be created.
pub(crate) fn create_private_folder(folder: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;

        std::fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(folder)
    }
    #[cfg(not(unix))]
    {
        std::fs::create_dir_all(folder)
    }
}
