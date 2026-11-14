// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Platform base folders and the process override for application data.

use std::ffi::OsString;
use std::path::PathBuf;

/// The optional absolute root whose `config`, `cache` and `data` folders replace platform folders.
pub const FOLDERS_ROOT_VARIABLE: &str = "P4KX_FOLDERS_ROOT";

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

/// Returns the base folder of `kind`, honouring [`FOLDERS_ROOT_VARIABLE`] before platform defaults.
/// An explicitly empty or relative root returns `None` without falling back to the user profile.
#[must_use]
pub fn base_folder(kind: FolderKind) -> Option<PathBuf> {
    if let Some(root) = std::env::var_os(FOLDERS_ROOT_VARIABLE) {
        let root = PathBuf::from(root);
        if !root.is_absolute() {
            return None;
        }
        let folder = match kind {
            FolderKind::Config => "config",
            FolderKind::Cache => "cache",
            FolderKind::Data => "data",
        };
        return Some(root.join(folder));
    }
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
