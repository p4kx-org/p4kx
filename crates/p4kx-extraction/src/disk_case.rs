// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Finding a file on disk that a link names with other case than the file has (ADR-0332).
//!
//! Links inside game files match names ignoring ASCII case, as the Archive lookup does. A
//! case-sensitive file system needs the name the file really has, so a link resolves through the
//! folder listing when the exact name is missing.

use std::ffi::{OsStr, OsString};
use std::fs::{self, File};
use std::io;
use std::path::{Component, Path, PathBuf};

/// Returns the path that differs from `path` only in the ASCII case of its parts and names an
/// existing file or folder, or `None` when there is none or more than one candidate at a part.
///
/// A part that exists as written is kept without listing its folder; only a missing part reads
/// the folder above it.
///
/// # Errors
///
/// Returns the error of reading a folder, other than the folder being absent.
pub(crate) fn variant(path: &Path) -> io::Result<Option<PathBuf>> {
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
        match unique_match(&found, name) {
            Ok(Some(actual)) => found.push(actual),
            Ok(None) => return Ok(None),
            Err(error) if is_absent(&error) => return Ok(None),
            Err(error) => return Err(error),
        }
    }
    Ok(Some(found))
}

/// Opens `path` through [`variant`], returning the file with the path it was opened at, or `None`
/// when no file matches.
///
/// # Errors
///
/// Returns the error of listing a folder or of opening the file.
pub(crate) fn open_variant(path: &Path) -> io::Result<Option<(File, PathBuf)>> {
    let Some(found) = variant(path)? else {
        return Ok(None);
    };
    match File::open(&found) {
        Ok(file) => Ok(Some((file, found))),
        Err(error) if is_absent(&error) => Ok(None),
        Err(error) => Err(error),
    }
}

/// Returns the name in `folder` that equals `name` ignoring ASCII case, when exactly one does.
fn unique_match(folder: &Path, name: &OsStr) -> io::Result<Option<OsString>> {
    let folder = if folder.as_os_str().is_empty() {
        Path::new(".")
    } else {
        folder
    };
    let mut matching = None;
    for item in fs::read_dir(folder)? {
        let actual = item?.file_name();
        if actual.eq_ignore_ascii_case(name) {
            if matching.is_some() {
                return Ok(None);
            }
            matching = Some(actual);
        }
    }
    Ok(matching)
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

    use super::{open_variant, variant};

    #[test]
    fn a_name_with_other_case_finds_the_file_by_its_real_name() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::create_dir(dir.path().join("Textures"))?;
        let real = dir.path().join("Textures").join("Rock_Diff.dds");
        fs::write(&real, b"DDS ")?;

        let asked = dir.path().join("textures").join("ROCK_diff.DDS");
        let found = variant(&asked)?.ok_or(std::io::ErrorKind::NotFound)?;

        assert_eq!(fs::read(&found)?, b"DDS ");
        if cfg!(unix) {
            assert_eq!(found, real, "the name the file has");
        }
        let (mut file, _) = open_variant(&asked)?.ok_or(std::io::ErrorKind::NotFound)?;
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

        assert_eq!(variant(&real)?.as_deref(), Some(real.as_path()));
        Ok(())
    }

    #[test]
    fn a_missing_name_or_folder_finds_nothing() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        fs::write(dir.path().join("a.txt"), b"x")?;

        assert_eq!(variant(&dir.path().join("b.txt"))?, None);
        assert_eq!(variant(&dir.path().join("nope").join("a.txt"))?, None);
        assert_eq!(variant(&dir.path().join("a.txt").join("x"))?, None);
        assert!(open_variant(&dir.path().join("B.TXT"))?.is_none());
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

        assert_eq!(variant(&dir.path().join("same.txt"))?, None);
        assert!(variant(&dir.path().join("Same.txt"))?.is_some(), "exact");
        Ok(())
    }
}
