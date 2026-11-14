// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    AnimationInfo, ArchiveModelAnimations, ArchivePositionedAnimation, FieldOrigins, FieldPath,
};

use crate::content_origin::EntryPageSources;
use crate::error::ToolError;
use crate::model_animation_navigation::Navigation;
use crate::model_animation_row_origins::{ItemUses, RowSites};
use crate::model_animation_sites::SkeletonSites;
use crate::model_item_page::{self, EvidenceError, ModelItemPage};
use crate::model_shapes::{ModelItemOrigins, ModelItemsResult};
use crate::models;

pub(crate) fn list(request: &ModelItemPage<'_>, path: &str) -> Result<ModelItemsResult, ToolError> {
    let index = request.opened.index();
    let read = ArchiveModelAnimations::read(&index, request.source.name())
        .map_err(|error| models::converted(path, "a model with its animations", error))?;
    let clips = read.clips();
    let render = |navigation| {
        let values = request.page.slice(clips).iter().map(|clip| {
            let info = AnimationInfo::of(
                request.source.model(),
                clip.animation().animation(),
                clip.source_name(),
            );
            serde_json::to_value(info).map_err(ToolError::from)
        });
        request.publish(clips.len(), values, |range, cursor| {
            field_origins(request, clips, range, cursor, navigation)
        })
    };
    match render(Navigation::Complete(read.admissions())) {
        Err(ToolError::TooLargeItem { .. }) if !read.admissions().is_empty() => {
            render(Navigation::witness(read.admissions()))
        }
        result => result,
    }
}

fn field_origins(
    request: &ModelItemPage<'_>,
    clips: &[ArchivePositionedAnimation],
    range: Range<usize>,
    cursor: Option<&str>,
    navigation: Navigation<'_>,
) -> Result<ModelItemOrigins, EvidenceError> {
    let model =
        EntryPageSources::from_origin(request.primary().and_then(|source| source.origin()))?;
    let page = clips.get(range).ok_or(EvidenceError::MissingRows)?;
    let skeleton = if page
        .iter()
        .any(|clip| !clip.animation().animation().tracks.is_empty())
    {
        SkeletonSites::of(request, &model)?
    } else {
        SkeletonSites {
            uses: Vec::new(),
            complete: true,
        }
    };
    let mut fields = FieldOrigins::new();
    let root = FieldPath::parse("/structuredContent")?;
    for (name, origin) in [
        ("archive", model.local()),
        ("model", model.path.clone()),
        ("section", model.local()),
    ] {
        model_item_page::put(&mut fields, &root, name, origin)?;
    }
    let mut aggregate = ItemUses::of(&model)?;
    let mut path = root.clone();
    path.push_field("items")?;
    for (index, clip) in page.iter().enumerate() {
        path.push_index(index)?;
        let entry = EntryPageSources::from_origin_into(clip.origin(), model.uses.clone())?;
        let receipt = RowSites::new(&entry, clip)?;
        receipt.write(&mut fields, &path, &skeleton, &mut aggregate)?;
        path.pop();
    }
    model_item_page::put(&mut fields, &root, "items", aggregate.finish(&model))?;
    let navigation = navigation.origin(&model)?;
    model_item_page::put(&mut fields, &root, "total", navigation.clone())?;
    if cursor.is_some() {
        model_item_page::put(&mut fields, &root, "next_cursor", navigation)?;
    }
    Ok(ModelItemOrigins {
        field_origins: fields,
        provenance: model.uses.freeze()?,
    })
}
