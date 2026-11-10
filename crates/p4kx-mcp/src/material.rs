// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::MaterialLibrary;
use p4kx_extraction::{Archive, find_entry, read_bytes};

use crate::budget::{Budget, fill_within, json_len};
use crate::container::Place;
use crate::error::ToolError;
use crate::material_shape;
use crate::material_shapes;
pub(crate) use crate::material_shapes::{MaterialParams, MaterialsResult};
use crate::page::Page;

/// The largest `.mtl` Entry read, in bytes.
pub(crate) const MAX_MTL_LEN: u64 = 8 << 20;

/// Returns one page of the sub-materials of the `.mtl` Entry at `path` in `place`.
pub(crate) fn summary(
    place: &Place<'_>,
    path: &str,
    page: Page,
) -> Result<MaterialsResult, ToolError> {
    let archive = place.archive();
    let (name, library) = read(archive, path)?;
    let mut text = Budget::new();
    let archive_path = archive.path().display().to_string();
    text.admit_text(archive_path.len())?;
    let shown_path = place.shown(&name);
    text.admit_text(shown_path.len())?;
    let mut result = MaterialsResult {
        archive: archive_path,
        path: shown_path,
        single: library.single,
        default_palette: library
            .default_palette
            .as_deref()
            .map(|value| text.text(value))
            .transpose()?,
        unknown_entries: library.unknown_entries,
        total: library.materials.len(),
        materials: Vec::new(),
        next_cursor: None,
    };
    let items = page
        .slice(&library.materials)
        .iter()
        .enumerate()
        .map(|(position, material)| material_shape::shape(page.offset() + position, material));
    let (materials, next_cursor) = fill_within(
        page,
        library.materials.len(),
        json_len(&result),
        items,
        |shape: &material_shapes::MaterialShape| format!("material {}", shape.id),
    )?;
    result.materials = materials;
    result.next_cursor = next_cursor;
    Ok(result)
}

fn read(archive: &Archive, path: &str) -> Result<(String, MaterialLibrary), ToolError> {
    let entry =
        find_entry(archive, path).ok_or_else(|| ToolError::UnknownEntry(path.to_owned()))?;
    if entry.uncompressed_size() > MAX_MTL_LEN {
        return Err(ToolError::Refused(format!(
            "the Entry {} holds {} bytes, more than the {MAX_MTL_LEN} a material library may",
            entry.path(),
            entry.uncompressed_size()
        )));
    }
    let len = usize::try_from(entry.uncompressed_size()).unwrap_or(usize::MAX);
    let data = read_bytes(archive, entry, 0, len).map_err(|source| ToolError::Read {
        path: entry.path().to_owned(),
        source,
    })?;
    let library = MaterialLibrary::parse(entry.path(), &data.bytes).map_err(|source| {
        ToolError::Material {
            path: entry.path().to_owned(),
            source,
        }
    })?;
    Ok((entry.path().to_owned(), library))
}
