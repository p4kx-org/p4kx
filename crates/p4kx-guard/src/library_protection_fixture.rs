// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use p4kx_channels::{DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

pub(super) struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    pub(super) fn new() -> TestResult<Self> {
        Ok(Self {
            root: tempfile::tempdir()?,
        })
    }

    pub(super) fn root(&self) -> &Path {
        self.root.path()
    }

    pub(super) fn prefix(&self, name: &str) -> TestResult<PathBuf> {
        let prefix = self.root().join(name);
        fs::create_dir_all(prefix.join("drive_c"))?;
        Ok(prefix)
    }

    pub(super) fn log(prefix: &Path, user: &str) -> TestResult<PathBuf> {
        let folder = prefix
            .join("drive_c/users")
            .join(user)
            .join("AppData/Roaming/rsilauncher/logs");
        fs::create_dir_all(&folder)?;
        Ok(folder.join("log.log"))
    }

    pub(super) fn library(prefix: &Path, name: &str) -> TestResult<PathBuf> {
        let folder = prefix.join("drive_c/Games").join(name);
        fs::create_dir_all(folder.join("LIVE/Bin64"))?;
        Ok(folder)
    }

    pub(super) fn recipe(prefixes: Vec<PathBuf>) -> SearchRecipe {
        SearchRecipe::explicit(SearchPaths {
            wine_prefixes: prefixes,
            ..SearchPaths::default()
        })
    }

    pub(super) fn dynamic(&self) -> SearchRecipe {
        SearchRecipe::with_environment(
            SearchPaths::default(),
            WineEnvironment::new(Some(self.root().to_path_buf()), None),
            DiscoveryLimits::default(),
        )
    }
}

pub(super) fn libraries(log: &Path, names: &[&str]) -> TestResult {
    use std::fmt::Write as _;

    let mut text = String::new();
    for name in names {
        writeln!(
            &mut text,
            "Installing Star Citizen LIVE at C:\\\\Games\\\\{name} (force DP)",
        )?;
    }
    fs::write(log, text)?;
    Ok(())
}

/// Creates an owned temporary directory alias for a native binding test.
#[cfg(unix)]
pub(super) fn link_directory(link: &Path, target: &Path) -> TestResult {
    std::os::unix::fs::symlink(target, link)?;
    Ok(())
}

/// Creates an owned temporary directory alias for a native binding test.
#[cfg(windows)]
pub(super) fn link_directory(link: &Path, target: &Path) -> TestResult {
    let link: PathBuf = link.components().collect();
    let target: PathBuf = target.components().collect();
    let output = std::process::Command::new("cmd")
        .args(["/C", "mklink", "/J"])
        .arg(link)
        .arg(target)
        .output()?;
    if !output.status.success() {
        return Err(format!(
            "cannot create synthetic junction: {}",
            String::from_utf8_lossy(&output.stderr)
        )
        .into());
    }
    Ok(())
}

/// Removes only the owned temporary directory alias.
#[cfg(unix)]
pub(super) fn remove_directory_link(link: &Path) -> TestResult {
    fs::remove_file(link)?;
    Ok(())
}

/// Removes only the owned temporary directory alias.
#[cfg(windows)]
pub(super) fn remove_directory_link(link: &Path) -> TestResult {
    fs::remove_dir(link)?;
    Ok(())
}
