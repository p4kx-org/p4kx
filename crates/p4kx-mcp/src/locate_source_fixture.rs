// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs::{self, File, FileTimes};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use p4kx_channels::{DiscoveryLimits, PublicPatches, SearchPaths, SearchRecipe, WineEnvironment};

pub(super) type TestResult<T = ()> = Result<T, Box<dyn Error>>;

const FIXTURE_EPOCH_SECONDS: u64 = 1_792_000_000;

pub(super) struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    pub(super) fn new() -> TestResult<Self> {
        Ok(Self {
            root: tempfile::tempdir()?,
        })
    }

    pub(super) fn path(&self) -> &Path {
        self.root.path()
    }

    pub(super) fn prefix(&self, name: &str) -> TestResult<PathBuf> {
        let prefix = self.path().join(name);
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

    pub(super) fn explicit(prefixes: Vec<PathBuf>) -> SearchRecipe {
        SearchRecipe::explicit(SearchPaths {
            wine_prefixes: prefixes,
            ..SearchPaths::default()
        })
    }

    pub(super) fn environment(&self, steam: &Path) -> SearchRecipe {
        let environment =
            WineEnvironment::new(None, Some(self.path().join("prefix").into_os_string()))
                .with_steam_roots(vec![steam.to_path_buf()]);
        SearchRecipe::with_environment(
            SearchPaths::default(),
            environment,
            DiscoveryLimits::default(),
        )
    }
}

pub(super) fn patch(path: &Path, version: &str) -> TestResult {
    fs::write(
        path,
        format!("Delta update applied (SC LIVE {version}-live.10000001)\n"),
    )?;
    Ok(())
}

pub(super) fn mtime(path: &Path) -> TestResult<SystemTime> {
    Ok(fs::metadata(path)?.modified()?)
}

pub(super) fn set_mtime(path: &Path, time: SystemTime) -> TestResult {
    File::options()
        .write(true)
        .open(path)?
        .set_times(FileTimes::new().set_modified(time))?;
    Ok(())
}

pub(super) fn time(seconds: u64) -> SystemTime {
    UNIX_EPOCH + Duration::from_secs(FIXTURE_EPOCH_SECONDS + seconds)
}

pub(super) fn assert_patch(patches: &PublicPatches, expected: Option<&str>) {
    assert_eq!(
        patches.get("10000001"),
        expected,
        "the current source map names the requested Build"
    );
}
