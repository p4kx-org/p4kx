// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Finding a file on disk that a link names with other case than the file has (ADR-0332).
//!
//! Links inside game files match names ignoring ASCII case, as the Archive lookup does. A
//! case-sensitive file system needs the name the file really has, so a link resolves through the
//! folder listing when the exact name is missing.

use std::collections::HashMap;
use std::ffi::OsString;
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

/// The names of the folders listed so far, by ASCII lower case, so a folder is read once however
/// many links miss in it.
#[derive(Debug, Default)]
pub(crate) struct CaseIndex {
    folders: HashMap<PathBuf, Option<HashMap<String, Vec<OsString>>>>,
}

impl CaseIndex {
    /// Returns the path that differs from `path` only in the ASCII case of its parts and names an
    /// existing file or folder, or `None` when there is none, more than one candidate at a part,
    /// or a folder cannot be listed.
    ///
    /// A part that exists as written is kept without listing; the first part that is missing
    /// takes the one file of that name ignoring case, with no way back to another spelling of an
    /// earlier part.
    pub(crate) fn variant(&mut self, path: &Path) -> Option<PathBuf> {
        let mut found = PathBuf::new();
        for component in path.components() {
            let Component::Normal(name) = component else {
                found.push(component);
                continue;
            };
            let exact = found.join(name);
            if fs::symlink_metadata(&exact).is_ok() {
                found = exact;
                continue;
            }
            let actual = self.unique_match(&found, name)?;
            found.push(actual);
        }
        Some(found)
    }

    /// Opens `path` through [`Self::variant`], returning the file with the path it was opened at,
    /// or `None` when no file matches.
    ///
    /// # Errors
    ///
    /// Returns the error of opening the matching file, other than it being absent.
    pub(crate) fn open_variant(&mut self, path: &Path) -> io::Result<Option<(File, PathBuf)>> {
        let Some(found) = self.variant(path) else {
            return Ok(None);
        };
        match File::open(&found) {
            Ok(file) => Ok(Some((file, found))),
            Err(error) if is_absent(&error) => Ok(None),
            Err(error) => Err(error),
        }
    }

    fn unique_match(&mut self, folder: &Path, name: &std::ffi::OsStr) -> Option<OsString> {
        let listing = self
            .folders
            .entry(folder.to_path_buf())
            .or_insert_with(|| list(folder))
            .as_ref()?;
        let mut candidates = listing
            .get(&key(name))?
            .iter()
            .filter(|actual| actual.eq_ignore_ascii_case(name));
        let only = candidates.next()?;
        candidates.next().is_none().then(|| only.clone())
    }
}

fn key(name: &std::ffi::OsStr) -> String {
    name.to_string_lossy().to_ascii_lowercase()
}

/// Lists `folder` by lower-case name; `None` when it cannot be read.
fn list(folder: &Path) -> Option<HashMap<String, Vec<OsString>>> {
    let folder = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };
    let mut names: HashMap<String, Vec<OsString>> = HashMap::new();
    for item in fs::read_dir(folder).ok()? {
        let actual = item.ok()?.file_name();
        names.entry(key(&actual)).or_default().push(actual);
    }
    Some(names)
}

fn is_absent(error: &io::Error) -> bool {
    matches!(
        error.kind(),
        io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
    )
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::io::Read;

    use super::CaseIndex;

    #[test]
    fn a_name_with_other_case_finds_the_file_by_its_real_name() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("Textures"))?;
        let real = dir.path().join("Textures").join("Rock_Diff.dds");
        fs::write(&real, b"DDS ")?;

        let asked = dir.path().join("textures").join("ROCK_diff.DDS");
        let mut index = CaseIndex::default();
        let found = index.variant(&asked).ok_or(std::io::ErrorKind::NotFound)?;

        assert_eq!(fs::read(&found)?, b"DDS ");
        if cfg!(target_os = "linux") {
            assert_eq!(found, real, "the name the file has");
        }
        let (mut file, _) = index
            .open_variant(&asked)?
            .ok_or(std::io::ErrorKind::NotFound)?;
        let mut text = String::new();
        file.read_to_string(&mut text)?;
        assert_eq!(text, "DDS ");
        Ok(())
    }

    #[test]
    fn a_name_that_exists_as_written_is_returned_unchanged() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let real = dir.path().join("a.txt");
        fs::write(&real, b"x")?;

        assert_eq!(
            CaseIndex::default().variant(&real).as_deref(),
            Some(real.as_path())
        );
        Ok(())
    }

    #[test]
    fn a_missing_name_or_folder_finds_nothing() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("a.txt"), b"x")?;
        let mut index = CaseIndex::default();

        assert_eq!(index.variant(&dir.path().join("b.txt")), None);
        assert_eq!(index.variant(&dir.path().join("nope").join("a.txt")), None);
        assert_eq!(index.variant(&dir.path().join("a.txt").join("x")), None);
        assert!(index.open_variant(&dir.path().join("B.TXT"))?.is_none());
        Ok(())
    }

    #[test]
    fn a_listing_is_read_once_and_kept() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("Late.txt"), b"x")?;
        let mut index = CaseIndex::default();
        assert!(index.variant(&dir.path().join("late.TXT")).is_some());

        fs::write(dir.path().join("Newer.txt"), b"x")?;

        let seen = index.variant(&dir.path().join("newer.txt"));
        assert!(
            seen.is_none() || cfg!(windows),
            "the first listing is reused: {seen:?}"
        );
        Ok(())
    }

    #[test]
    fn two_names_differing_only_in_case_are_ambiguous() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("Same.txt"), b"1")?;
        if fs::write(dir.path().join("SAME.TXT"), b"2").is_err()
            || fs::read_dir(dir.path())?.count() < 2
        {
            return Ok(());
        }
        let mut index = CaseIndex::default();

        assert_eq!(index.variant(&dir.path().join("same.txt")), None);
        assert!(
            index.variant(&dir.path().join("Same.txt")).is_some(),
            "exact"
        );
        Ok(())
    }
}
