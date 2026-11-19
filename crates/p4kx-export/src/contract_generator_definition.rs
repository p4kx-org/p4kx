// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Filter, RecordTree};

use crate::build::original_input::OriginalCatalogInput;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_read::original::OriginalFlowFields;
use crate::mission_generator::{GeneratorDefinition, completion_tag_results, item_reward_results};
use crate::record_read::ReadError;
use crate::source_fields::{FieldCapture, FieldError};
use crate::source_value_catalog::SourceArenaOutput;
use crate::{CatalogError, Kind};

pub(crate) struct DefinitionProjection<'source> {
    pub(crate) definition: GeneratorDefinition<'source>,
    pub(crate) item_error: Option<ReadError>,
    pub(crate) notes: Vec<FieldError>,
}

pub(crate) fn matches_path(path: &str, filter: Option<&Filter>) -> bool {
    Kind::Contracts.records().matches_path(path) && filter.is_none_or(|filter| filter.matches(path))
}

pub(crate) fn read<'source>(
    input: &OriginalCatalogInput<'_, 'source>,
    tree: &RecordTree<'source>,
    fields: &OriginalFlowFields<'_, '_, 'source>,
    source: SourceArenaOutput<'source>,
    capture: &dyn FieldCapture<'source>,
) -> Result<DefinitionProjection<'source>, CatalogError> {
    let mut notes = Vec::new();
    let completion_tag_results = completion_tag_results::read(fields, capture, &mut notes)?;
    let (mission_item_properties, item_error) =
        match crate::mission_generator::item_properties::read(fields, capture) {
            Ok(items) => (items, None),
            Err(error) => (Vec::new(), Some(error)),
        };
    Ok(DefinitionProjection {
        definition: GeneratorDefinition {
            input_ref: fields.input_ref().clone(),
            header: CanonicalRecordHeader::read(tree, input.origins())?,
            source,
            completion_tag_results,
            item_reward_results: item_reward_results::read(fields)?,
            mission_item_properties,
            hauling_target_bindings: crate::location_hauling_targets::read(
                fields,
                fields.facts().iter(),
                |fact| Ok(crate::mission_generator::value_ref(fact)),
            )?,
        },
        item_error,
        notes,
    })
}
