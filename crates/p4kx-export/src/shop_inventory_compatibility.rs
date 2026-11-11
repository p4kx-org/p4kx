// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeSet;

use p4kx_extraction::{CancelToken, SourceValueState};

use crate::shop_inventory_fields::ShopInventoryFields;
use crate::shop_inventory_source_read::{self as source, ParsedInventory, SourceNodeValue};
use crate::shop_registry;
use crate::{CatalogError, ExportAccountingError};

/// The set-like item projection of the original inventory document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ItemList {
    Items { guids: Vec<String>, dropped: usize },
    Missing,
    Unreadable(String),
}

pub(super) fn items(
    fields: &ShopInventoryFields,
    parsed: &ParsedInventory<'_>,
    cancel: Option<&CancelToken>,
) -> Result<Option<ItemList>, CatalogError> {
    if fields.inventory.state() == SourceValueState::Invalid
        || fields.collection.state() == SourceValueState::Invalid
    {
        return Ok(None);
    }
    let lines = fields.collection.value().map_or_else(
        || fields.inventory.value(),
        |collection| collection.inventory.value(),
    );
    let Some(lines) = lines else {
        return Ok(None);
    };
    let mut guids = BTreeSet::new();
    let mut dropped = 0;
    for line in lines {
        source::check_cancel(cancel)?;
        let alternatives = line
            .value()
            .and_then(|line| line.id.value())
            .and_then(|id| id.alternatives.value());
        let Some(alternatives) = alternatives else {
            dropped += 1;
            continue;
        };
        for alternative in alternatives {
            source::check_cancel(cancel)?;
            let Some(reference) = alternative.value() else {
                dropped += 1;
                continue;
            };
            let node = parsed.node(source::index(*reference)?)?;
            let SourceNodeValue::String(text) = &node.value else {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            };
            if let Some(text) = text.text().filter(|text| shop_registry::is_guid(text)) {
                guids.insert(text.to_ascii_lowercase());
            } else {
                dropped += 1;
            }
        }
    }
    Ok(Some(ItemList::Items {
        guids: guids.into_iter().collect(),
        dropped,
    }))
}

pub(super) fn why_unparsed(bytes: &[u8]) -> &'static str {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        "the file is empty"
    } else {
        "the file is no inventory JSON"
    }
}
