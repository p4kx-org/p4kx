// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use crate::MAX_SETTINGS_SIZE;
use crate::key_store::create_private;
use crate::locations::{APP_FOLDER as FOLDER, FolderKind, base_folder};

const PREVIOUS_FOLDER: &str = "P4K-Extractor";
const STAGING_PREFIX: &str = "P4KX.migrating-";
const KNOWN_FILES: [&str; 2] = ["settings.toml", "archive.key"];
const MAX_DEPTH: usize = 8;
const MAX_ENTRIES: usize = 1024;
const MAX_FILE_LEN: u64 = MAX_SETTINGS_SIZE;
const MAX_TOTAL_LEN: u64 = 4 * MAX_SETTINGS_SIZE;

static FOLDER_IN_USE: OnceLock<Option<PathBuf>> = OnceLock::new();

/// What [`migrate_profile`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProfileMigration {
    /// The `P4KX` folder exists already; nothing was copied.
    NotNeeded,
    /// Neither folder exists, or the old one is not a folder; the start is fresh.
    NothingToMigrate,
    /// The old folder was copied completely into a new `P4KX` folder.
    Migrated,
    /// The old folder could not be copied completely; no `P4KX` folder was created.
    Failed(io::ErrorKind),
}

/// Returns the profile folder of the platform config directory, `None` without one.
///
/// It is `P4KX` (`%APPDATA%\P4KX` on Windows, `$XDG_CONFIG_HOME/P4KX` on Linux), or the old
/// `P4K-Extractor` folder for this process when its migration failed. The first call of a
/// process runs [`migrate_profile`]; its outcome never stops the start.
#[must_use]
pub fn profile_folder() -> Option<PathBuf> {
    FOLDER_IN_USE
        .get_or_init(|| base_folder(FolderKind::Config).map(|config| profile_folder_in(&config)))
        .clone()
}

/// Runs [`migrate_profile`] for `config` and returns the folder to use.
///
/// It is `P4KX`, or the old `P4K-Extractor` folder when the migration failed, so that nothing is
/// written into a fresh `P4KX` that would hide the old folder from the next start's migration.
#[must_use]
pub fn profile_folder_in(config: &Path) -> PathBuf {
    match migrate_profile(config) {
        ProfileMigration::Failed(_) => config.join(PREVIOUS_FOLDER),
        _ => config.join(FOLDER),
    }
}

/// Copies the `P4K-Extractor` folder of `config` to `P4KX` when the latter does not exist yet.
///
/// The old folder is never changed; when it is a link to a folder, the link is followed. The copy
/// starts with `settings.toml` and `archive.key`, goes through a staging folder and is renamed to
/// `P4KX` only when complete, so a file in an existing `P4KX` folder is never overwritten and a
/// failed copy leaves no `P4KX` folder. Links inside the folder and `.tmp` files are skipped. A
/// known file that cannot be copied, a file above 16 MiB, or more than 1024 entries, 8 levels
/// or 64 MiB make the migration [`ProfileMigration::Failed`]. Leftover staging folders of earlier
/// runs are removed once `P4KX` exists.
#[must_use]
pub fn migrate_profile(config: &Path) -> ProfileMigration {
    let target = config.join(FOLDER);
    if target.symlink_metadata().is_ok() {
        remove_staging_folders(config);
        return ProfileMigration::NotNeeded;
    }
    let source = config.join(PREVIOUS_FOLDER);
    match source.metadata() {
        Ok(found) if found.is_dir() => {}
        Ok(_) => return ProfileMigration::NothingToMigrate,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return ProfileMigration::NothingToMigrate;
        }
        Err(error) => return ProfileMigration::Failed(error.kind()),
    }
    let staging = config.join(format!("{STAGING_PREFIX}{}", std::process::id()));
    let _ignored = std::fs::remove_dir_all(&staging);
    let copied = std::fs::create_dir(&staging).and_then(|()| copy_profile(&source, &staging));
    let published = copied.and_then(|()| {
        if target.symlink_metadata().is_ok() {
            return Err(io::Error::from(io::ErrorKind::AlreadyExists));
        }
        std::fs::rename(&staging, &target)
    });
    match published {
        Ok(()) => {
            remove_staging_folders(config);
            ProfileMigration::Migrated
        }
        Err(error) => {
            let _ignored = std::fs::remove_dir_all(&staging);
            if error.kind() == io::ErrorKind::AlreadyExists {
                remove_staging_folders(config);
                ProfileMigration::NotNeeded
            } else {
                ProfileMigration::Failed(error.kind())
            }
        }
    }
}

fn remove_staging_folders(config: &Path) {
    let Ok(entries) = std::fs::read_dir(config) else {
        return;
    };
    for entry in entries.flatten() {
        let is_staging = entry
            .file_name()
            .to_string_lossy()
            .starts_with(STAGING_PREFIX);
        if is_staging && entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let _ignored = std::fs::remove_dir_all(entry.path());
        }
    }
}

#[derive(Default)]
struct Budget {
    entries: usize,
    bytes: u64,
}

fn limit(what: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, what)
}

fn copy_profile(from: &Path, to: &Path) -> io::Result<()> {
    let mut budget = Budget::default();
    for name in KNOWN_FILES {
        let file = from.join(name);
        match file.symlink_metadata() {
            Ok(found) if found.is_file() => {
                budget.entries += 1;
                copy_file(&file, &to.join(name), &mut budget)?;
            }
            Ok(_) => return Err(limit("a known profile file is not a regular file")),
            Err(error) if error.kind() == io::ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
    }
    copy_folder(from, to, 0, &mut budget)
}

fn copy_folder(from: &Path, to: &Path, depth: usize, budget: &mut Budget) -> io::Result<()> {
    for entry in std::fs::read_dir(from)? {
        let entry = entry?;
        let name = entry.file_name();
        let kind = entry.file_type()?;
        if name.to_string_lossy().ends_with(".tmp")
            || (depth == 0 && KNOWN_FILES.iter().any(|known| name == *known))
        {
            continue;
        }
        if kind.is_dir() {
            if depth >= MAX_DEPTH {
                return Err(limit("the profile folder is nested too deeply"));
            }
            budget.entries += 1;
            if budget.entries > MAX_ENTRIES {
                return Err(limit("the profile folder has too many entries"));
            }
            let destination = to.join(&name);
            std::fs::create_dir(&destination)?;
            copy_folder(&entry.path(), &destination, depth + 1, budget)?;
        } else if kind.is_file() {
            budget.entries += 1;
            if budget.entries > MAX_ENTRIES {
                return Err(limit("the profile folder has too many entries"));
            }
            copy_file(&entry.path(), &to.join(&name), budget)?;
        }
    }
    Ok(())
}

fn copy_file(from: &Path, to: &Path, budget: &mut Budget) -> io::Result<()> {
    let source = std::fs::File::open(from)?;
    let len = source.metadata()?.len();
    if len > MAX_FILE_LEN {
        return Err(limit("a profile file is too large"));
    }
    if budget.bytes.saturating_add(len) > MAX_TOTAL_LEN {
        return Err(limit("the profile folder is too large"));
    }
    let mut target = create_private(to)?;
    let copied = io::copy(&mut io::Read::take(source, MAX_FILE_LEN), &mut target)?;
    target.sync_all()?;
    budget.bytes = budget.bytes.saturating_add(copied);
    Ok(())
}
