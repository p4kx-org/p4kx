// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::catalog_v3::catalog_item_components::item_definition::ItemAttachmentPresentation;

/// Paint matching inputs from the selected original attachment presentation.
#[derive(Debug, Default)]
pub(crate) struct ItemPaintFacts<'source> {
    pub(crate) item_type: Option<&'source str>,
    pub(crate) tags: Vec<&'source str>,
    pub(crate) required_tags: Vec<&'source str>,
}

impl<'source> ItemPaintFacts<'source> {
    pub(super) fn read(value: Option<&ItemAttachmentPresentation<'source>>) -> Self {
        let Some(value) = value else {
            return Self::default();
        };
        let item_type = value
            .item_type()
            .value()
            .copied()
            .filter(|value| !value.is_empty());
        let tags = words(value.tags().value().copied());
        let required_tags = if item_type.is_some_and(crate::paint::is_paint_type) {
            words(value.required_tags().value().copied())
        } else {
            Vec::new()
        };
        Self {
            item_type,
            tags,
            required_tags,
        }
    }
}

fn words(value: Option<&str>) -> Vec<&str> {
    value.map_or_default(|tags| tags.split_whitespace().collect())
}
