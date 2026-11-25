// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::ModelBoneSources;
use p4kx_extraction::model_info::primary_skeleton_selection_uses;
use p4kx_extraction::{SourceEvidenceError, SourceLocation, SourceScope, SourceUse, SourceUseRole};

use crate::content_origin::EntryPageSources;
use crate::model_item_page::ModelItemPage;

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
        let (mut uses, complete) =
            primary_skeleton_selection_uses(model.payload_scope(), survey, skeleton)?;
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
