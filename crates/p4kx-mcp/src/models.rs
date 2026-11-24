// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{LodSelection, MaterialReading, PositionBox};
use p4kx_extraction::{
    AnimationChoice, AnimationSet, ConversionError, LibraryInfo, ModelSource, TextureVerdicts,
};

use crate::archives::Opened;
use crate::budget::{MAX_RESPONSE_LEN, json_len};
use crate::error::ToolError;
use crate::model_item_page::ModelItemPage;
use crate::model_material_items;
use crate::model_mesh_items;
use crate::model_node_items;
use crate::model_params::Section;
use crate::model_shapes::{LodShape, ModelCounts, ModelItemsResult, ModelOverview};
use crate::page::Page;
use crate::{model_animation_items, model_bone_items};

#[path = "model_chunk_items.rs"]
mod chunk_items;

#[path = "model_overview_origins.rs"]
mod overview_origins;

#[path = "model_unread_animation_items.rs"]
mod unread_animation_items;

/// Returns the summary of the model Entry at `path`.
pub(crate) fn overview(opened: &Opened, path: &str) -> Result<ModelOverview, ToolError> {
    let (source, animations) = read(opened, path, ModelRead::Overview)?;
    let model = source.model();
    let library = serde_json::to_value(LibraryInfo::of(model))?;
    let material_ids = model.material_ids();
    let origins = overview_origins::capture(&source, &library, &material_ids)?;
    let overview = ModelOverview {
        archive: opened.archive().path().display().to_string(),
        model: source.name().to_owned(),
        geometry: model.geometry.clone(),
        material_library: model.material_library.clone(),
        material_ids,
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
        origins,
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
    let (source, _) = read(opened, path, ModelRead::Section(section))?;
    let what = section.name();
    let reserved = envelope_len(opened, source.name(), what);
    let observed = ModelItemPage {
        opened,
        source: &source,
        section,
        page,
        reserved,
    };
    match section {
        Section::Materials => model_material_items::list(&observed),
        Section::Nodes => model_node_items::list(&observed),
        Section::Meshes => model_mesh_items::list(&observed),
        Section::Bones => model_bone_items::list(&observed),
        Section::Animations => model_animation_items::list(&observed, path),
        Section::UnreadAnimations => unread_animation_items::list(&observed, path),
        Section::Chunks => chunk_items::list(&observed),
    }
}

fn envelope_len(opened: &Opened, model: &str, section: &str) -> usize {
    json_len(&(
        opened.archive().path().display().to_string(),
        model,
        section,
    ))
    .saturating_add(128)
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
    if matches!(request, ModelRead::Section(Section::Materials)) {
        let source =
            ModelSource::from_index_with_material_pages(&index, path, &TextureVerdicts::new())
                .map_err(|error| converted(path, "a model with its material sources", error))?;
        return Ok((source, AnimationSet::default()));
    }
    let verdicts = TextureVerdicts::new();
    let source = if matches!(request, ModelRead::Overview) {
        ModelSource::from_index_with_material_summary_origins(&index, path, &verdicts)
    } else {
        let load = match request {
            ModelRead::Section(Section::Bones | Section::Animations) => {
                ModelSource::from_index_observing_model_sources
            }
            ModelRead::Section(Section::Nodes) => ModelSource::from_index_observing_nodes,
            _ => ModelSource::from_index_reading,
        };
        load(
            &index,
            path,
            LodSelection::All,
            PositionBox::default(),
            MaterialReading::Plain,
            &verdicts,
        )
    }
    .map_err(|error| converted(path, "a model", error))?;
    let animations = if matches!(request, ModelRead::Overview) {
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
