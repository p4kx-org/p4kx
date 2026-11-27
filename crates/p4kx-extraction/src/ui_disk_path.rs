// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::DiskTextures;

const DDS: &str = "dds";
const DATA: &str = "Data";

/// Distinguishes confined image-path refusals from filesystem failures.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DiskPathError {
    #[error("external image leaves {}", root.display())]
    Outside { root: PathBuf },
    #[error("could not read {}: {source}", path.display())]
    Read {
        path: PathBuf,
        #[source]
        source: io::Error,
    },
}

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
/// Returns a confinement refusal or the original filesystem failure.
pub(crate) fn resolve_on_disk(
    movie: &Path,
    relative: &str,
    files: &DiskTextures,
) -> Result<Option<PathBuf>, DiskPathError> {
    let folder = std::path::absolute(movie)
        .map_err(|source| read_error(movie, source))?
        .parent()
        .map_or_default(Path::to_path_buf);
    let root = image_root(&folder);
    if relative.contains(':') || relative.starts_with(['/', '\\']) {
        return Err(DiskPathError::Outside { root });
    }
    let mut path = folder;
    for part in relative.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                if path == root || !path.pop() {
                    return Err(DiskPathError::Outside { root });
                }
            }
            part => path.push(part),
        }
    }
    if !path.starts_with(&root) {
        return Err(DiskPathError::Outside { root });
    }
    let Some(path) = files
        .resolve_path(&path)
        .map_err(|source| read_error(&path, source))?
    else {
        return Ok(None);
    };
    let canonical = fs::canonicalize(&path).map_err(|source| read_error(&path, source))?;
    let canonical_root = fs::canonicalize(&root).map_err(|source| read_error(&root, source))?;
    if canonical.starts_with(&canonical_root) {
        Ok(Some(canonical))
    } else {
        Err(DiskPathError::Outside { root })
    }
}

fn read_error(path: &Path, source: io::Error) -> DiskPathError {
    DiskPathError::Read {
        path: path.to_path_buf(),
        source,
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::{DiskPathError, dds_candidate};
    use crate::DiskTextures;

    fn resolve_on_disk(
        movie: &Path,
        relative: &str,
    ) -> Result<Option<std::path::PathBuf>, DiskPathError> {
        super::resolve_on_disk(movie, relative, &DiskTextures::default())
    }

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
        assert!(
            matches!(
                resolve_on_disk(&movie, "../../Textures/missing.dds"),
                Ok(None)
            ),
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
                matches!(&result, Err(DiskPathError::Outside { root: found })
                    if fs::canonicalize(found).ok() == root),
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

    #[cfg(unix)]
    #[test]
    fn an_unusable_directory_is_an_io_error_instead_of_outside_or_missing() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let movie = dir.path().join("m.gfx");
        touch(&movie)?;
        let parent = dir.path().join("plain");
        touch(&parent)?;
        let requested = parent.join("x.dds");
        let expected = fs::metadata(&requested).expect_err("file in directory position");

        let result = resolve_on_disk(&movie, "plain/x.dds");

        assert!(
            matches!(&result, Err(DiskPathError::Read { path, source })
            if path == &requested && source.raw_os_error() == expected.raw_os_error()),
            "{result:?}"
        );
        Ok(())
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_cannot_leave_the_image_root() -> std::io::Result<()> {
        let dir = tempfile::tempdir()?;
        let movie = dir.path().join("Data/m.gfx");
        touch(&movie)?;
        let outside = dir.path().join("outside.dds");
        touch(&outside)?;
        std::os::unix::fs::symlink(&outside, dir.path().join("Data/link.dds"))?;

        assert!(matches!(
            resolve_on_disk(&movie, "link.dds"),
            Err(DiskPathError::Outside { .. })
        ));
        Ok(())
    }
}
