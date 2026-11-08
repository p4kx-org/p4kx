// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::ExternalImage;

use crate::ui_disk_path::{self, DiskPathError};
use crate::ui_movie::Origin;
use crate::{ConversionError, DiskTextures, Texture, UiMovie};

/// Opens the DDS texture the external image `image` of the definition `id` names, looked up
/// relative to the movie in its Archive or, confined to the movie's `Data` folder, on disk.
pub(crate) fn open_external<'a>(
    movie: &UiMovie<'a>,
    id: u16,
    image: &ExternalImage,
    files: &DiskTextures,
) -> Result<Texture<'a>, ConversionError> {
    let file_name = if image.file_name.is_empty() {
        &image.export_name
    } else {
        &image.file_name
    };
    let missing = |tried| ConversionError::ExternalImageMissing {
        movie: movie.name().to_owned(),
        id,
        file: file_name.clone(),
        tried,
    };
    match movie.origin() {
        Origin::Entry(archive) => {
            let named = [file_name.clone()];
            let dds = ui_disk_path::dds_candidate(file_name);
            let candidates: Vec<&String> = named.iter().chain(dds.iter()).collect();
            let mut tried = Vec::new();
            let mut last_error = None;
            for candidate in candidates {
                let Some(path) = entry_path(movie.name(), candidate) else {
                    continue;
                };
                match Texture::from_archive(archive, &path) {
                    Ok(texture) => return Ok(texture),
                    Err(ConversionError::UnknownEntry(_)) => {}
                    Err(error) => last_error = Some(error),
                }
                tried.push(path);
            }
            Err(last_error.unwrap_or_else(|| missing(tried)))
        }
        Origin::File(file) => {
            let name = ui_disk_path::dds_candidate(file_name).unwrap_or_else(|| file_name.clone());
            let path =
                ui_disk_path::resolve_on_disk(file, &name, files).map_err(|error| match error {
                    DiskPathError::Outside { root } => ConversionError::ExternalImageOutside {
                        movie: movie.name().to_owned(),
                        id,
                        file: file_name.clone(),
                        root,
                    },
                    DiskPathError::Read { path, source } => ConversionError::Read { path, source },
                })?;
            path.map_or_else(|| Err(missing(vec![name])), |path| files.open(&path))
        }
    }
}

/// Returns the Entry path `relative` names from the folder of the Entry `movie`, or `None` when
/// it is absolute or leaves the Archive.
pub(crate) fn entry_path(movie: &str, relative: &str) -> Option<String> {
    if relative.contains(':') || relative.starts_with(['/', '\\']) {
        return None;
    }
    let mut parts: Vec<&str> = movie.split(['/', '\\']).collect();
    parts.pop();
    for part in relative.split(['/', '\\']) {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            part => parts.push(part),
        }
    }
    Some(parts.join("\\"))
}

#[cfg(test)]
mod tests {
    use super::entry_path;

    #[test]
    fn entry_paths_resolve_against_the_movie_folder_inside_the_archive() {
        assert_eq!(
            entry_path(
                "Data\\UI\\Screens\\assets\\SWF\\m.gfx",
                "../../../Textures/./x.dds"
            )
            .as_deref(),
            Some("Data\\UI\\Textures\\x.dds"),
            "dot segments"
        );
        assert_eq!(
            entry_path("Data\\UI\\m.gfx", "../../../x.dds"),
            None,
            "leaves"
        );
        assert_eq!(entry_path("Data\\UI\\m.gfx", "C:\\x.dds"), None, "drive");
        assert_eq!(entry_path("Data\\UI\\m.gfx", "/x.dds"), None, "absolute");
    }
}
