// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{LodSelection, MaterialReading, PositionBox};
use p4kx_extraction::{
    AnimationChoice, AnimationInfo, AnimationSet, ChunkInfo, ConversionError, LibraryInfo, LodInfo,
    MaterialInfo, MeshInfo, ModelSource, NodeInfo, TextureVerdicts, UnreadInfo,
};
use serde::Serialize;

use crate::archives::Opened;
use crate::budget::{MAX_RESPONSE_LEN, fill_within, json_len};
use crate::error::ToolError;
use crate::model_bone_items;
use crate::model_params::Section;
use crate::model_shapes::{LodShape, ModelCounts, ModelItemsResult, ModelOverview};
use crate::page::Page;

/// Returns the summary of the model Entry at `path`.
pub(crate) fn overview(opened: &Opened, path: &str) -> Result<ModelOverview, ToolError> {
    let (source, animations) = read(opened, path, ModelRead::Overview)?;
    let model = source.model();
    let library = serde_json::to_value(LibraryInfo::of(model))?;
    let overview = ModelOverview {
        archive: opened.archive().path().display().to_string(),
        model: source.name().to_owned(),
        geometry: model.geometry.clone(),
        material_library: model.material_library.clone(),
        material_ids: model.material_ids(),
        library,
        counts: ModelCounts {
            materials: model
                .materials
                .library()
                .map_or(0, |library| library.materials.len()),
            nodes: model.nodes.len(),
            lods: model.lods.len(),
            meshes: model.lods.iter().map(|lod| lod.meshes.len()).sum(),
            bones: model.bones.len(),
            animations: animations.animations.len(),
            unread_animations: animations.unread.len(),
            chunks: model.chunks.len(),
        },
        lods: model
            .lods
            .iter()
            .map(|lod| LodShape {
                level: lod.level,
                vertices: lod.vertex_count(),
                triangles: lod.triangle_count(),
                meshes: lod.meshes.len(),
            })
            .collect(),
    };
    if json_len(&overview) > MAX_RESPONSE_LEN {
        return Err(ToolError::TooLargeItem {
            what: format!("summary of the model {}", overview.model),
            limit: MAX_RESPONSE_LEN,
            hint: "its material IDs or levels of detail are too many for one response",
        });
    }
    Ok(overview)
}

/// Returns one page of the section `section` of the model Entry at `path`; only that section is
/// built, and the animations are read only for the two sections that list them.
pub(crate) fn items(
    opened: &Opened,
    path: &str,
    section: Section,
    page: Page,
) -> Result<ModelItemsResult, ToolError> {
    let (source, animations) = read(opened, path, ModelRead::Section(section))?;
    let model = source.model();
    let what = section.name();
    let reserved = envelope_len(opened, source.name(), what);
    let (total, items, next_cursor) = match section {
        Section::Materials => paged(
            &MaterialInfo::of_model(model),
            page,
            what,
            reserved,
            |item| item,
        )?,
        Section::Nodes => paged(&NodeInfo::of_model(model), page, what, reserved, |item| {
            item
        })?,
        Section::Meshes => {
            let lods = LodInfo::of_model(model);
            let meshes: Vec<LodMesh<'_>> = lods
                .iter()
                .flat_map(|lod| {
                    lod.meshes.iter().map(|mesh| LodMesh {
                        lod: lod.level,
                        mesh,
                    })
                })
                .collect();
            paged(&meshes, page, what, reserved, |item| item)?
        }
        Section::Bones => return model_bone_items::list(opened, &source, page, reserved),
        Section::Animations => {
            let read: Vec<_> = animations
                .animations
                .iter()
                .zip(&animations.origins)
                .collect();
            paged(&read, page, what, reserved, |&(animation, origin)| {
                AnimationInfo::of(model, animation, origin)
            })?
        }
        Section::UnreadAnimations => {
            paged(&animations.unread, page, what, reserved, UnreadInfo::of)?
        }
        Section::Chunks => paged(&ChunkInfo::of_model(model), page, what, reserved, |item| {
            item
        })?,
    };
    Ok(ModelItemsResult {
        archive: opened.archive().path().display().to_string(),
        model: source.name().to_owned(),
        section: what,
        total,
        items,
        next_cursor,
        origins: None,
    })
}

fn envelope_len(opened: &Opened, model: &str, section: &str) -> usize {
    json_len(&(
        opened.archive().path().display().to_string(),
        model,
        section,
    ))
    .saturating_add(128)
}

#[derive(Serialize)]
struct LodMesh<'a> {
    lod: u32,
    #[serde(flatten)]
    mesh: &'a MeshInfo,
}

type Paged = (usize, Vec<serde_json::Value>, Option<String>);

fn paged<'a, T, U: Serialize>(
    items: &'a [T],
    page: Page,
    what: &str,
    reserved: usize,
    convert: impl Fn(&'a T) -> U,
) -> Result<Paged, ToolError> {
    let values = page
        .slice(items)
        .iter()
        .map(|item| serde_json::to_value(convert(item)).map_err(ToolError::from));
    let (values, next) = fill_within(page, items.len(), reserved, values, |_| {
        format!("item of {what}")
    })?;
    Ok((items.len(), values, next))
}

#[derive(Clone, Copy)]
enum ModelRead {
    Overview,
    Section(Section),
}

fn read(
    opened: &Opened,
    path: &str,
    request: ModelRead,
) -> Result<(ModelSource, AnimationSet), ToolError> {
    let index = opened.index();
    let load = if matches!(request, ModelRead::Section(Section::Bones)) {
        ModelSource::from_index_observing_model_sources
    } else {
        ModelSource::from_index_reading
    };
    let source = load(
        &index,
        path,
        LodSelection::All,
        PositionBox::default(),
        MaterialReading::Plain,
        &TextureVerdicts::new(),
    )
    .map_err(|error| converted(path, "a model", error))?;
    let with_animations = match request {
        ModelRead::Overview => true,
        ModelRead::Section(section) => section.needs_animations(),
    };
    let animations = if with_animations {
        AnimationSet::from_archive(&index, source.name(), &AnimationChoice::All, &[])
            .map_err(|error| converted(path, "a model with its animations", error))?
    } else {
        AnimationSet::default()
    };
    Ok((source, animations))
}

pub(crate) fn converted(path: &str, what: &'static str, error: ConversionError) -> ToolError {
    match error {
        ConversionError::UnknownEntry(unknown) => ToolError::UnknownEntry(
            unknown
                .0
                .first()
                .map_or_else(|| path.to_owned(), Clone::clone),
        ),
        error => ToolError::Convert {
            path: path.to_owned(),
            what,
            source: Box::new(error),
        },
    }
}

#[cfg(test)]
mod tests {
    use p4kx_extraction::{ConversionError, UnknownEntries};

    use super::converted;
    use crate::error::ToolError;

    #[test]
    fn an_unknown_entry_error_names_the_path_it_carries() {
        let error = ConversionError::from(UnknownEntries(vec!["Data/other.cgfm".to_owned()]));
        assert!(
            matches!(
                converted("Data/model.cgf", "a model", error),
                ToolError::UnknownEntry(path) if path == "Data/other.cgfm"
            ),
            "the missing Entry, not the model"
        );
    }
}
