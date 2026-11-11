// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock, PoisonError, RwLock};

use p4kx_channels::{DiscoveryIssue, SearchRecipe};

use crate::{Installation, WriteProtectionError, WriteRefused};

#[path = "library_protection_cache.rs"]
mod cache;

use cache::LibraryProtectionCache;

const GAME_FOLDER: &str = "Bin64";
const EXTRA_FOLDERS_VARIABLE: &str = "P4KX_PROTECT_FOLDERS";

static PROTECTED: RwLock<Installation> = RwLock::new(Installation::new());
static LIBRARIES: Mutex<LibraryProtectionCache> = Mutex::new(LibraryProtectionCache::new());
static STANDARD_RECIPE: OnceLock<SearchRecipe> = OnceLock::new();
static CONFIGURED_RECIPE: RwLock<Option<Arc<SearchRecipe>>> = RwLock::new(None);

/// Shares the active profile's Channel search with installation protection.
pub fn configure_library_discovery(recipe: Arc<SearchRecipe>) {
    *CONFIGURED_RECIPE
        .write()
        .unwrap_or_else(PoisonError::into_inner) = Some(recipe);
}

/// Protects `folder` and everything below it for the rest of the process.
pub fn protect(folder: &Path) {
    PROTECTED
        .write()
        .unwrap_or_else(PoisonError::into_inner)
        .add(folder);
}

/// Protects the Library folders of this machine: the default one and those the RSI Launcher log
/// names. Call it once at program start, before any file is written.
///
/// The folders in the path list of the environment variable `P4KX_PROTECT_FOLDERS` are protected
/// as well. It is a test aid for fake installations and can only add protection.
pub fn protect_libraries() {
    let _issues = protect_libraries_report();
}

/// Protects discovered Library folders and returns incomplete-discovery diagnostics.
#[must_use]
pub fn protect_libraries_report() -> Vec<DiscoveryIssue> {
    let configured = CONFIGURED_RECIPE
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .clone();
    let recipe = configured
        .as_deref()
        .unwrap_or_else(|| STANDARD_RECIPE.get_or_init(SearchRecipe::standard));
    let snapshot = recipe.snapshot();
    let issues = LIBRARIES
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .refresh(
            &snapshot,
            &mut PROTECTED.write().unwrap_or_else(PoisonError::into_inner),
        );
    if let Some(extra) = std::env::var_os(EXTRA_FOLDERS_VARIABLE) {
        for folder in std::env::split_paths(&extra).filter(|folder| !folder.as_os_str().is_empty())
        {
            protect(&folder);
        }
    }
    issues
}

/// Protects the Library folders of this machine and, when it holds a game installation, the
/// folder of the Archive at `archive`.
///
/// The Archive's folder is a game installation when it holds the game's `Bin64` folder; a copy of
/// an Archive elsewhere is not one. Discovery refreshes the source topology before reusing a
/// complete unchanged launcher-log snapshot.
pub fn protect_archive_folder(archive: &Path) {
    let _issues = protect_archive_folder_report(archive);
}

/// Protects libraries and the Archive's installation, returning library-discovery diagnostics.
///
/// Library discovery runs once; an incomplete report still protects the Archive's installation.
#[must_use]
pub fn protect_archive_folder_report(archive: &Path) -> Vec<DiscoveryIssue> {
    let issues = protect_libraries_report();
    protect_installation_of(archive);
    issues
}

/// Protects the folders the user entered by hand that are an installation, and the folder of each
/// Archive entered by hand that is one.
///
/// A folder is an installation when it holds the game's `Bin64` folder itself (a Channel folder)
/// or has a child folder that does (a Library folder); any other folder stays writable. Call it
/// with the manual entries of the settings whenever they are loaded or extended.
pub fn protect_manual(folders: &[PathBuf], archives: &[PathBuf]) {
    for folder in folders {
        if is_installation(folder) {
            protect(folder);
        }
    }
    for archive in archives {
        protect_installation_of(archive);
    }
}

fn is_installation(folder: &Path) -> bool {
    folder.join(GAME_FOLDER).is_dir()
        || std::fs::read_dir(folder).is_ok_and(|items| {
            items
                .filter_map(Result::ok)
                .any(|item| item.path().join(GAME_FOLDER).is_dir())
        })
}

fn protect_installation_of(archive: &Path) {
    if let Some(folder) = std::path::absolute(archive)
        .ok()
        .and_then(|absolute| absolute.parent().map(Path::to_path_buf))
        && folder.join(GAME_FOLDER).is_dir()
    {
        protect(&folder);
    }
}

/// Checks that writing to `target` stays outside the protected folders.
///
/// # Errors
///
/// Returns an established installation refusal or unavailable target identity.
pub fn ensure_writable(target: &Path) -> Result<(), WriteProtectionError> {
    let host =
        crate::write_target::host_target(target, &std::sync::atomic::AtomicBool::new(false))?;
    check_known(&host).map_err(Into::into)
}

pub(crate) fn check_known(target: &Path) -> Result<(), WriteRefused> {
    PROTECTED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .check(target)
}

pub(crate) fn check_known_namespace(target: &Path) -> Result<(), WriteRefused> {
    PROTECTED
        .read()
        .unwrap_or_else(PoisonError::into_inner)
        .check_namespace(target)
}
