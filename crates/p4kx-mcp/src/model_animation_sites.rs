// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::model::{AnimationChunkCoordinates, ModelBoneSources};
use p4kx_extraction::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

use crate::content_origin::EntryPageSources;
use crate::model_bone_items;
use crate::model_item_page::ModelItemPage;

pub(crate) struct ChunkSites {
    pub(crate) selectors: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl ChunkSites {
    pub(crate) fn of(
        scope: &SourceScope,
        chunk: &AnimationChunkCoordinates,
    ) -> Result<Self, SourceEvidenceError> {
        let mut selectors = Vec::new();
        for (field, start, end) in [
            ("signature", 0, 4),
            ("rowCount", 8, 12),
            ("tableOffset", 12, 16),
        ] {
            selectors.push(site(
                scope,
                "cryEngineChunkHeader",
                field,
                vec![start, end],
                SourceUseRole::Selector,
            )?);
        }
        if let Some(prefix) = chunk.type_prefix() {
            let first = prefix.first_word();
            selectors.push(site(
                scope,
                "cryEngineChunkTable",
                "animationTypePrefix",
                vec![
                    first.start,
                    first.end,
                    prefix.stride(),
                    prefix.count() as u64,
                ],
                SourceUseRole::Selector,
            )?);
        }
        if let Some(row) = chunk.row() {
            for (field, bytes) in [
                ("type", &row.kind),
                ("version", &row.version),
                ("offset", &row.offset),
            ] {
                selectors.push(site(
                    scope,
                    "cryEngineChunkTable",
                    field,
                    vec![chunk.ordinal() as u64, bytes.start, bytes.end],
                    SourceUseRole::Selector,
                )?);
            }
        }
        Ok(Self {
            selectors,
            complete: chunk.row().is_some() && chunk.type_prefix().is_some(),
        })
    }
}

pub(crate) fn clip_site(
    entry: &EntryPageSources,
    clip: usize,
    chunk: usize,
    field: &str,
    bytes: Range<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    site(
        entry.payload_scope(),
        "cryEngineAnimation",
        field,
        vec![clip as u64, chunk as u64, bytes.start, bytes.end],
        role,
    )
}

pub(crate) fn frame_site(
    entry: &EntryPageSources,
    clip: usize,
    chunk: usize,
    controller: usize,
    channel: &str,
    bytes: Range<u64>,
) -> Result<SourceUse, SourceEvidenceError> {
    site(
        entry.payload_scope(),
        "cryEngineAnimationFrame",
        channel,
        vec![
            clip as u64,
            chunk as u64,
            controller as u64,
            bytes.start,
            bytes.end,
        ],
        SourceUseRole::Conversion,
    )
}

pub(crate) struct SkeletonSites {
    pub(crate) uses: Vec<SourceUse>,
    pub(crate) complete: bool,
}

impl SkeletonSites {
    pub(crate) fn of(
        request: &ModelItemPage<'_>,
        model: &EntryPageSources,
    ) -> Result<Self, SourceEvidenceError> {
        let primary = request.primary();
        let survey = primary.and_then(|source| source.primary_skeleton());
        let skeleton = primary.and_then(|source| source.bone_sources());
        let (mut uses, complete) = model_bone_items::primary_selection(model, survey, skeleton)?;
        let count = request.source.model().bones.len();
        let words = skeleton.and_then(|skeleton| controller_words(skeleton, count));
        let located = words.is_some();
        if let Some(words) = words {
            uses.push(site(
                model.payload_scope(),
                "cryEngineSkeleton",
                "controllerIds",
                words,
                SourceUseRole::Conversion,
            )?);
        }
        Ok(Self {
            uses,
            complete: model.complete() && complete && (count == 0 || located),
        })
    }
}

fn controller_words(skeleton: &ModelBoneSources, count: usize) -> Option<Vec<u64>> {
    let first = skeleton.bone(0)?.controller_id();
    let stride = if count == 1 {
        first.end - first.start
    } else {
        skeleton
            .bone(1)?
            .controller_id()
            .start
            .checked_sub(first.start)?
    };
    for ordinal in 0..count {
        let shift = stride.checked_mul(ordinal as u64)?;
        let range = skeleton.bone(ordinal)?.controller_id();
        if range != (first.start.checked_add(shift)?..first.end.checked_add(shift)?) {
            return None;
        }
    }
    Some(vec![
        skeleton.chunk() as u64,
        first.start,
        first.end,
        stride,
        count as u64,
    ])
}

fn site(
    scope: &SourceScope,
    namespace: &str,
    field: &str,
    indices: Vec<u64>,
    role: SourceUseRole,
) -> Result<SourceUse, SourceEvidenceError> {
    scope.at(
        SourceLocation::Subresource {
            namespace: namespace.into(),
            field: field.into(),
            indices,
        },
        role,
    )
}
