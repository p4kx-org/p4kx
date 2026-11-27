// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{MaterialLibrary, MaterialReadError, MaterialSourceLimits};
use p4kx_extraction::{Archive, Entry, find_entry, read_bytes};

use crate::budget::{self, Budget};
use crate::container::Place;
use crate::error::ToolError;
use crate::material_origin::Observations;
use crate::material_shape;
use crate::material_shapes::{MaterialHeader, MaterialShape};
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
    let entry =
        find_entry(archive, path).ok_or_else(|| ToolError::UnknownEntry(path.to_owned()))?;
    let mut observations = Observations::default();
    let library = read(archive, entry, page, &mut observations)?;
    let header = header(place, entry, &library)?;
    let shapes = page
        .slice(&library.materials)
        .iter()
        .enumerate()
        .map(|(position, material)| material_shape::shape(page.offset() + position, material));
    let (mut shapes, _) = budget::fill_within(
        page,
        header.total,
        budget::json_len(&header),
        shapes,
        |shape: &MaterialShape| format!("material {}", shape.id),
    )?;
    let (keep, result) = budget::fit_prefix(
        shapes.len(),
        |length| {
            let materials = shapes.split_at(length).0;
            let next_cursor = page.next_cursor_after(length, header.total)?;
            let (field_origins, provenance) =
                observations.project(archive, entry, materials, &header, next_cursor.as_deref())?;
            Ok(MaterialsResult {
                header: &header,
                materials,
                next_cursor,
                field_origins,
                provenance,
            })
        },
        || {
            shapes.first().map_or_else(
                || "result without its list".to_owned(),
                |shape| format!("material {}", shape.id),
            )
        },
    )?;
    let MaterialsResult {
        next_cursor,
        field_origins,
        provenance,
        ..
    } = result;
    shapes.truncate(keep);
    Ok(MaterialsResult {
        header,
        materials: shapes,
        next_cursor,
        field_origins,
        provenance,
    })
}

fn header(
    place: &Place<'_>,
    entry: &Entry,
    library: &MaterialLibrary,
) -> Result<MaterialHeader, ToolError> {
    let mut text = Budget::new();
    let archive = place.archive().path().display().to_string();
    text.admit_text(archive.len())?;
    let path = place.shown(entry.path());
    text.admit_text(path.len())?;
    Ok(MaterialHeader {
        archive,
        path,
        single: library.single,
        default_palette: library
            .default_palette
            .as_deref()
            .map(|value| text.text(value))
            .transpose()?,
        unknown_entries: library.unknown_entries,
        total: library.materials.len(),
    })
}

fn read(
    archive: &Archive,
    entry: &Entry,
    page: Page,
    observations: &mut Observations,
) -> Result<MaterialLibrary, ToolError> {
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
    let mut limits = MaterialSourceLimits::default();
    limits.source_bytes = data.bytes.len().max(1);
    MaterialLibrary::parse_with_observer(
        entry.path(),
        &data.bytes,
        limits,
        || Ok(()),
        |site| observations.retain(page, site),
    )
    .map_err(|error| match error {
        MaterialReadError::Material(source) => ToolError::Material {
            path: entry.path().to_owned(),
            source,
        },
        MaterialReadError::Interrupted(source) => ToolError::MaterialEvidence(source),
        other => ToolError::MaterialRead {
            path: entry.path().to_owned(),
            source: Box::new(other),
        },
    })
}
