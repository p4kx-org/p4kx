// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::path::{Path, PathBuf};

use crate::disk_case::CaseIndex;

const DDS: &str = "dds";
const DATA: &str = "Data";

/// Returns `file_name` with the extension `.dds` instead of its own, or `None` when it already
/// ends in `.dds`; the game ships the textures the GFx exporter refers to as DDS.
pub(crate) fn dds_candidate(file_name: &str) -> Option<String> {
    let name_start = file_name.rfind(['/', '\\']).map_or(0, |at| at + 1);
    let (stem, extension) = file_name
        .rfind('.')
        .filter(|&at| at > name_start)
        .and_then(|at| file_name.split_at_checked(at))
        .map_or((file_name, ""), |(stem, dot_extension)| {
            (stem, dot_extension.get(1..).unwrap_or_default())
        });
    (!extension.eq_ignore_ascii_case(DDS)).then(|| format!("{stem}.{DDS}"))
}

/// Returns the folder a movie file's external images must lie in: its nearest ancestor folder
/// named `Data`, ignoring case, or else the movie's own folder.
pub(crate) fn image_root(folder: &Path) -> PathBuf {
    folder
        .ancestors()
        .find(|ancestor| {
            ancestor
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.eq_ignore_ascii_case(DATA))
        })
        .unwrap_or(folder)
        .to_path_buf()
}

/// Resolves the image file `relative` names from the folder of the movie file `movie`.
///
/// The path is normalised lexically and must stay inside [`image_root`]; an existing file is
/// canonicalised and checked against the root again, so a link cannot leave it either.
///
/// # Errors
///
/// Returns the root when the path is absolute, carries a drive or leaves the root.
pub(crate) fn resolve_on_disk(movie: &Path, relative: &str) -> Result<Option<PathBuf>, PathBuf> {
    let folder = std::path::absolute(movie)
        .ok()
        .and_then(|movie| movie.parent().map(Path::to_path_buf))
        .unwrap_or_default();
    let root = image_root(&folder);
    if relative.contains(':') || relative.starts_with(['/', '\\']) {
        return Err(root);
    }
    let mut path = folder;
    for part in relative.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                if path == root || !path.pop() {
                    return Err(root);
                }
            }
            part => path.push(part),
        }
    }
    if !path.starts_with(&root) {
        return Err(root);
    }
    if !path.is_file() {
        if cfg!(windows) {
            return Ok(None);
        }
        match CaseIndex::default().variant(&path) {
            Some(found) if found.is_file() => path = found,
            _ => return Ok(None),
        }
    }
    let canonical = fs::canonicalize(&path).map_err(|_| root.clone())?;
    let canonical_root = fs::canonicalize(&root).map_err(|_| root.clone())?;
    if canonical.starts_with(&canonical_root) {
        Ok(Some(canonical))
    } else {
        Err(root)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{dds_candidate, resolve_on_disk};

    #[test]
    fn the_dds_candidate_replaces_the_last_extension() {
        assert_eq!(
            dds_candidate("../Textures/bg.png.tif").as_deref(),
            Some("../Textures/bg.png.dds"),
            "last extension"
        );
        assert_eq!(dds_candidate("Textures\\Button.DDS"), None, "already DDS");
        assert_eq!(
            dds_candidate("a.b/plain").as_deref(),
            Some("a.b/plain.dds"),
            "no extension"
        );
    }

    fn touch(path: &Path) -> std::io::Result<()> {
        fs::create_dir_all(path.parent().unwrap_or(path))?;
        fs::write(path, b"DDS ")
    }

    #[test]
    fn paths_stay_inside_the_nearest_data_folder() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let data = dir.path().join("game").join("data");
        let movie = data.join("UI").join("SWF").join("m.gfx");
        touch(&movie)?;
        touch(&data.join("Textures").join("x.dds"))?;
        touch(&dir.path().join("game").join("secret.dds"))?;
        let found = resolve_on_disk(&movie, "../../Textures/./x.dds");
        assert!(
            matches!(&found, Ok(Some(path)) if path.ends_with("x.dds")),
            "{found:?}"
        );
        assert_eq!(
            resolve_on_disk(&movie, "../../Textures/missing.dds"),
            Ok(None),
            "missing"
        );
        let root = fs::canonicalize(&data).ok();
        for refused in [
            "../../../secret.dds",
            "..\\..\\..\\..\\..\\..\\Windows\\x.dds",
            "C:\\x.dds",
            "/x.dds",
            "\\\\server\\share\\x.dds",
        ] {
            let result = resolve_on_disk(&movie, refused);
            assert!(
                matches!(&result, Err(found) if fs::canonicalize(found).ok() == root),
                "{refused}: {result:?}"
            );
        }
        Ok(())
    }

    #[test]
    fn without_a_data_folder_paths_stay_in_the_movie_folder() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let movie = dir.path().join("screens").join("m.gfx");
        touch(&movie)?;
        touch(&dir.path().join("screens").join("Textures").join("x.dds"))?;
        touch(&dir.path().join("y.dds"))?;
        assert!(
            matches!(resolve_on_disk(&movie, "Textures/x.dds"), Ok(Some(_))),
            "below"
        );
        assert!(resolve_on_disk(&movie, "../y.dds").is_err(), "above");
        Ok(())
    }

    #[test]
    fn a_link_with_other_case_finds_the_file_it_names() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let data = dir.path().join("Data");
        let movie = data.join("UI").join("m.gfx");
        touch(&movie)?;
        touch(&data.join("Textures").join("Button.dds"))?;

        let found = resolve_on_disk(&movie, "../textures/BUTTON.DDS");

        assert!(
            matches!(&found, Ok(Some(path)) if path.ends_with("Textures/Button.dds")),
            "{found:?}"
        );
        Ok(())
    }
}
