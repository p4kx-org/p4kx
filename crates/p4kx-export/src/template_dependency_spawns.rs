// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::RecordTree;

use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::tag_catalog::CapturedTags;
use crate::record_observation::RecordObservation;
use crate::record_read::ReadError;
use crate::spawn_layers::Level;
use crate::spawn_model::SpawnLevel;
use crate::spawn_pools::SpawnPools;
use crate::spawn_read::Reader;
use crate::template_field_capture::TemplateFieldCapture;
use crate::template_reference_index::TemplateReferences;
use crate::template_spawn_context::TemplateSpawnContext;
use crate::template_tag_cache::TemplateTagCache;

/// Runs the existing template dependency recipe against its genuine original positions.
pub(crate) fn read<'source>(
    input: &OriginalCatalogInput<'_, '_>,
    tags: &CapturedTags<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    pools: &mut SpawnPools<'source>,
) -> Result<Level, ReadError> {
    let capture = TemplateFieldCapture::new(fields, input, admission)?;
    let references = RefCell::new(TemplateReferences::new(fields));
    let tag_cache = TemplateTagCache::new(fields, tags);
    let context = TemplateSpawnContext::new(fields, &references, &tag_cache, None, &capture)?;
    let reader = Reader::for_context(tags.tree(), tree, &context)?;
    Level::read(
        SpawnLevel::Template,
        &reader,
        (Some(tree.root()), "contractProperties"),
        (None, ""),
        pools,
    )
}
