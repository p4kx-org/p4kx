// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldOrigin, SourceValueState, SourcedValue};

use super::inputs::selected_attachment;
use super::paint_facts::ItemPaintFacts;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
use crate::guid_text::CatalogGuid;
use crate::{CatalogError, ExportAccountingError};

pub(crate) struct PaintInput<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) is_paint: bool,
    pub(crate) tags: Vec<&'source str>,
    pub(crate) required_tags: Vec<&'source str>,
    pub(crate) origin: FieldOrigin,
}

pub(crate) fn read<'source>(
    definition: &ItemDefinition<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    tree: &p4kx_extraction::RecordTree<'source>,
) -> Result<PaintInput<'source>, CatalogError> {
    let attachment_ordinal = ItemSourceSelection::read_attachment(tree, originals)?;
    let attachment = selected_attachment(definition, &attachment_ordinal)?;
    let header = definition.source();
    let guid = header
        .guid
        .value()
        .copied()
        .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
    let mut origins = vec![header.guid.origin(), attachment_ordinal.origin()];
    let mut complete = read_cell(&attachment_ordinal);
    let mut is_paint = false;
    let mut tags = Vec::new();
    let mut required_tags = Vec::new();
    if let Some(cell) = attachment {
        complete &= read_cell(cell);
        origins.push(cell.origin());
        if let Some(attachment) = cell.value() {
            let item_type = attachment.item_type();
            complete &= read_cell(item_type);
            origins.push(item_type.origin());
            is_paint = complete
                && item_type
                    .value()
                    .is_some_and(|value| crate::paint::is_paint_type(value));
            if is_paint {
                let offers = attachment.tags();
                let requires = attachment.required_tags();
                complete &= read_cell(offers) && read_cell(requires);
                origins.extend([offers.origin(), requires.origin()]);
                let facts = ItemPaintFacts::read(Some(attachment));
                tags = facts.tags;
                required_tags = facts.required_tags;
            }
        }
    } else {
        complete &= attachment_ordinal.state() == SourceValueState::Absent;
    }
    Ok(PaintInput {
        guid,
        is_paint,
        tags,
        required_tags,
        origin: originals.sources().origin_with_origins(origins, complete)?,
    })
}

fn read_cell<T>(value: &SourcedValue<T>) -> bool {
    value.origin().source_set_complete()
        && matches!(
            value.state(),
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        )
}
