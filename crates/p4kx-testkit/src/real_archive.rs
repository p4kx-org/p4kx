// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ffi::OsString;
use std::io::Read;
use std::path::{Path, PathBuf};

use p4kx_foundation::{ArchiveKey, FolderKind, base_folder};

#[cfg(test)]
#[path = "real_archive_profile_tests.rs"]
mod profile_tests;

/// The environment variable that holds the key of a real `Data.p4k` for the real-Archive suite.
pub const REAL_ARCHIVE_KEY_VAR: &str = "P4KX_ARCHIVE_KEY";

/// The environment variable that names a file holding the key, as `p4kx --key-file` does.
pub const REAL_ARCHIVE_KEY_FILE_VAR: &str = "P4KX_KEY_FILE";

const PROFILE_FOLDERS: [&str; 2] = ["P4KX", "P4K-Extractor"];
const PROFILE_FILE: &str = "archive.key";
const MAX_KEY_FILE_LEN: u64 = 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Outcome {
    Unset,
    NotThirtyTwoHexDigits,
    NoFile,
    Unreadable,
}

impl std::fmt::Display for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Unset => "not set",
            Self::NotThirtyTwoHexDigits => "not 32 hexadecimal digits",
            Self::NoFile => "no such file",
            Self::Unreadable => "cannot be read",
        })
    }
}

/// The real-Archive suite found no usable Archive key; it names the sources tried, never a key.
#[derive(Clone, PartialEq, Eq)]
pub struct MissingRealKey {
    tried: [(&'static str, Outcome); 3],
}

impl std::fmt::Debug for MissingRealKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::fmt::Display for MissingRealKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the real-Archive suite found no Archive key; tried ")?;
        for (index, (source, outcome)) in self.tried.iter().enumerate() {
            if index > 0 {
                f.write_str(", ")?;
            }
            write!(f, "{source} ({outcome})")?;
        }
        write!(
            f,
            "; set {REAL_ARCHIVE_KEY_VAR} or {REAL_ARCHIVE_KEY_FILE_VAR}, or store the key with `p4kx key set`"
        )
    }
}

impl std::error::Error for MissingRealKey {}

/// Returns the Archive key of the real-Archive suite, as the CLI and the GUI find it.
///
/// The first valid key wins: `P4KX_ARCHIVE_KEY`, the file named by `P4KX_KEY_FILE`, then the key
/// stored in the platform config folder, with `P4KX` before the legacy `P4K-Extractor`.
/// The shared folder resolver honours `P4KX_FOLDERS_ROOT`.
///
/// # Errors
///
/// Returns [`MissingRealKey`], naming the sources tried and never a key, when none of them
/// yields 32 hexadecimal digits.
pub fn real_archive_key() -> Result<ArchiveKey, MissingRealKey> {
    let variable = std::env::var(REAL_ARCHIVE_KEY_VAR).ok();
    let named = std::env::var_os(REAL_ARCHIVE_KEY_FILE_VAR)
        .filter(|path| !path.is_empty())
        .map(PathBuf::from);
    let paths = base_folder(FolderKind::Config)
        .map(|dir| PROFILE_FOLDERS.map(|folder| dir.join(folder).join(PROFILE_FILE)));
    let profile = paths
        .as_ref()
        .map(|paths| paths.each_ref().map(PathBuf::as_path));
    let profile_files: &[&Path] = match &profile {
        Some(paths) => paths,
        None => &[],
    };
    find_key(variable.as_deref(), named.as_deref(), profile_files)
}

fn read_profile_files(files: &[&Path]) -> Result<ArchiveKey, Outcome> {
    let mut outcome = Outcome::Unset;
    for file in files {
        match read_key_file(file) {
            Ok(key) => return Ok(key),
            Err(error) if outcome == Outcome::Unset || outcome == Outcome::NoFile => {
                outcome = error;
            }
            Err(_) => {}
        }
    }
    Err(outcome)
}

fn find_key(
    variable: Option<&str>,
    named_file: Option<&Path>,
    profile_files: &[&Path],
) -> Result<ArchiveKey, MissingRealKey> {
    let from_variable = variable
        .filter(|text| !text.trim().is_empty())
        .map_or(Err(Outcome::Unset), |text| {
            ArchiveKey::from_hex(text).map_err(|_| Outcome::NotThirtyTwoHexDigits)
        });
    let from_named = named_file.map_or(Err(Outcome::Unset), read_key_file);
    let from_profile = read_profile_files(profile_files);
    let mut found = None;
    let tried = [
        (REAL_ARCHIVE_KEY_VAR, from_variable),
        (REAL_ARCHIVE_KEY_FILE_VAR, from_named),
        ("the stored key", from_profile),
    ]
    .map(|(source, result)| match result {
        Ok(key) => {
            found.get_or_insert(key);
            (source, Outcome::Unset)
        }
        Err(outcome) => (source, outcome),
    });
    found.ok_or(MissingRealKey { tried })
}
fn read_key_file(file: &Path) -> Result<ArchiveKey, Outcome> {
    let opened = std::fs::File::open(file).map_err(|error| match error.kind() {
        std::io::ErrorKind::NotFound => Outcome::NoFile,
        _ => Outcome::Unreadable,
    })?;
    let mut text = String::new();
    opened
        .take(MAX_KEY_FILE_LEN)
        .read_to_string(&mut text)
        .map_err(|_| Outcome::Unreadable)?;
    ArchiveKey::from_hex(&text).map_err(|_| Outcome::NotThirtyTwoHexDigits)
}
/// The environment variable that names a real `Data.p4k` for the real-Archive suite.
pub const REAL_ARCHIVE_VAR: &str = "P4KX_DATA_P4K";

/// The environment variable that names a second real `Data.p4k` for Comparisons.
pub const REAL_ARCHIVE_B_VAR: &str = "P4KX_DATA_P4K_B";

/// The real-Archive suite was run without the variable naming the Archive it needs.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MissingRealArchive(&'static str);

impl std::fmt::Debug for MissingRealArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(self, f)
    }
}

impl std::fmt::Display for MissingRealArchive {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{} is not set; point it to a real Data.p4k to run the real-Archive suite",
            self.0
        )
    }
}

impl std::error::Error for MissingRealArchive {}

/// Returns the path of the real Archive named by `P4KX_DATA_P4K`.
///
/// # Errors
///
/// Returns [`MissingRealArchive`] when the variable is unset or empty.
pub fn real_archive_path() -> Result<PathBuf, MissingRealArchive> {
    non_empty(std::env::var_os(REAL_ARCHIVE_VAR)).ok_or(MissingRealArchive(REAL_ARCHIVE_VAR))
}

/// Returns the path of the second real Archive named by `P4KX_DATA_P4K_B`, `None` when the
/// variable is unset or empty, so that a test needing it can return early.
#[must_use]
pub fn real_archive_b_path() -> Option<PathBuf> {
    non_empty(std::env::var_os(REAL_ARCHIVE_B_VAR))
}

/// Returns the paths of the real Archives named by `P4KX_DATA_P4K` and `P4KX_DATA_P4K_B`, `None`
/// when the second is unset or empty, so that a test over two Archives can skip.
///
/// # Errors
///
/// Returns [`MissingRealArchive`] when `P4KX_DATA_P4K` is unset or empty.
pub fn real_archive_pair() -> Result<Option<(PathBuf, PathBuf)>, MissingRealArchive> {
    let first = real_archive_path()?;
    Ok(real_archive_b_path().map(|second| (first, second)))
}

fn non_empty(value: Option<OsString>) -> Option<PathBuf> {
    value.filter(|path| !path.is_empty()).map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;
    use std::path::PathBuf;

    use super::non_empty;

    #[test]
    fn an_unset_or_empty_variable_names_no_archive() {
        assert_eq!(non_empty(None), None, "unset");
        assert_eq!(non_empty(Some(OsString::new())), None, "empty");
    }

    #[test]
    fn a_set_variable_names_the_archive() {
        assert_eq!(
            non_empty(Some(OsString::from("b.p4k"))),
            Some(PathBuf::from("b.p4k")),
            "the path"
        );
    }

    use super::{MissingRealKey, find_key};

    const FIRST: &str = "000102030405060708090a0b0c0d0e0f";
    const SECOND: &str = "f0e0d0c0b0a090807060504030201000";

    fn key_file(directory: &tempfile::TempDir, name: &str, text: &str) -> PathBuf {
        let file = directory.path().join(name);
        std::fs::write(&file, text).expect("write the temporary key file");
        file
    }

    fn expected(text: &str) -> p4kx_foundation::ArchiveKey {
        p4kx_foundation::ArchiveKey::from_hex(text).expect("a synthetic key")
    }

    #[test]
    fn the_variable_comes_first() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let named = key_file(&directory, "named.key", SECOND);
        let profile = key_file(&directory, "profile.key", SECOND);
        let found = find_key(Some(FIRST), Some(&named), &[&profile]);
        assert_eq!(found.ok(), Some(expected(FIRST)), "the variable");
    }

    #[test]
    fn the_named_file_comes_before_the_profile() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let named = key_file(&directory, "named.key", &format!("{FIRST}\n"));
        let profile = key_file(&directory, "profile.key", SECOND);
        let found = find_key(Some("  "), Some(&named), &[&profile]);
        assert_eq!(found.ok(), Some(expected(FIRST)), "the named file");
    }

    #[test]
    fn the_profile_is_the_last_resort() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let profile = key_file(&directory, "profile.key", SECOND);
        let missing = directory.path().join("absent.key");
        let found = find_key(Some("not a key"), Some(&missing), &[&profile]);
        assert_eq!(found.ok(), Some(expected(SECOND)), "the profile file");
    }

    #[test]
    fn the_new_profile_folder_wins_and_the_old_one_is_the_fallback() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let new = key_file(&directory, "new.key", FIRST);
        let old = key_file(&directory, "old.key", SECOND);
        let absent = directory.path().join("absent.key");
        assert_eq!(
            find_key(None, None, &[&new, &old]).ok(),
            Some(expected(FIRST)),
            "both stored"
        );
        assert_eq!(
            find_key(None, None, &[&absent, &old]).ok(),
            Some(expected(SECOND)),
            "only the old folder"
        );
    }

    #[test]
    fn an_unusable_source_does_not_stop_the_search() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let named = key_file(&directory, "named.key", "short");
        let profile = key_file(&directory, "profile.key", FIRST);
        let found = find_key(None, Some(&named), &[&profile]);
        assert_eq!(found.ok(), Some(expected(FIRST)), "the profile file");
    }

    #[test]
    fn no_source_names_every_source_tried_and_no_key() {
        let directory = tempfile::tempdir().expect("a temporary folder");
        let named = key_file(&directory, "named.key", "short");
        let error: MissingRealKey =
            find_key(Some(SECOND.get(..31).unwrap_or("")), Some(&named), &[])
                .expect_err("no valid key");
        let message = error.to_string();
        assert!(
            message.contains("P4KX_ARCHIVE_KEY (not 32 hexadecimal digits)"),
            "{message}"
        );
        assert!(
            message.contains("P4KX_KEY_FILE (not 32 hexadecimal digits)"),
            "{message}"
        );
        assert!(message.contains("the stored key (not set)"), "{message}");
        assert!(!message.contains(&SECOND[..8]), "{message}");
        assert_eq!(format!("{error:?}"), message, "debug equals display");
    }
}
