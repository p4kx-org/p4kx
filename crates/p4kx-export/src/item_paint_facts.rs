// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};

use crate::catalog_v3::catalog_item_components::item_definition::ItemAttachmentPresentation;

/// Paint matching inputs from the selected original attachment presentation.
#[derive(Debug, Default)]
pub(crate) struct ItemPaintFacts<'source> {
    pub(crate) item_type: Option<&'source str>,
    pub(crate) tags: Vec<&'source str>,
    pub(crate) required_tags: Vec<&'source str>,
    pub(crate) source_set_complete: bool,
}

impl<'source> ItemPaintFacts<'source> {
    pub(super) fn selected(
        ordinal: &SourcedValue<u32>,
        value: Option<&SourcedValue<Box<ItemAttachmentPresentation<'source>>>>,
    ) -> Self {
        let mut facts = Self::read(value.and_then(SourcedValue::value).map(AsRef::as_ref));
        facts.source_set_complete &= read_cell(ordinal)
            && value.map_or_else(|| ordinal.state() == SourceValueState::Absent, read_cell);
        facts
    }
    pub(super) fn read(value: Option<&ItemAttachmentPresentation<'source>>) -> Self {
        let Some(value) = value else {
            return Self {
                source_set_complete: true,
                ..Self::default()
            };
        };
        let item_type = value
            .item_type()
            .value()
            .copied()
            .filter(|value| !value.is_empty());
        let tags = words(value.tags().value().copied());
        let mut source_set_complete = read_cell(value.item_type());
        let required_tags = if item_type.is_some_and(crate::paint::is_paint_type) {
            source_set_complete &= read_cell(value.tags()) && read_cell(value.required_tags());
            words(value.required_tags().value().copied())
        } else {
            Vec::new()
        };
        Self {
            item_type,
            tags,
            required_tags,
            source_set_complete,
        }
    }
}

pub(super) fn read_cell<T>(value: &SourcedValue<T>) -> bool {
    value.origin().source_set_complete()
        && matches!(
            value.state(),
            SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
        )
}

fn words(value: Option<&str>) -> Vec<&str> {
    value.map_or_default(|tags| tags.split_whitespace().collect())
}
