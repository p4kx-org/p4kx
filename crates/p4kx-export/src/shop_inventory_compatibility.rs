// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_extraction::{
    CancelToken, DecimalU64, FieldOrigin, SourceUses, SourceValue, SourceValueState, SourcedValue,
};

use crate::catalog_read::parsed_entry_json::{ParsedEntryJson, SourceNodeValue};
use crate::contract_shops::ShopItemGuids;
use crate::shop_inventory_fields::ShopInventoryFields;
use crate::shop_inventory_source_read as source;
use crate::shop_registry;
use crate::{CatalogError, ExportAccountingError};

/// The set-like item projection of the original inventory document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ItemList {
    Items {
        guids: SourcedValue<ShopItemGuids>,
        dropped: usize,
    },
    Missing,
    EmptySource(String),
    Unreadable(String),
}

impl ItemList {
    pub(crate) fn unreadable_reason(&self) -> Option<&str> {
        match self {
            Self::EmptySource(reason) | Self::Unreadable(reason) => Some(reason),
            Self::Items { .. } | Self::Missing => None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_items(&self) -> Option<(Vec<&str>, usize)> {
        match self {
            Self::Items { guids, dropped } => guids.value().map(|values| {
                (
                    values
                        .0
                        .iter()
                        .filter_map(SourcedValue::value)
                        .map(String::as_str)
                        .collect(),
                    *dropped,
                )
            }),
            _ => None,
        }
    }
}

pub(super) fn items(
    fields: &ShopInventoryFields,
    parsed: &ParsedEntryJson<'_>,
    sources: &SourceUses,
    cancel: Option<&CancelToken>,
) -> Result<Option<ItemList>, CatalogError> {
    if fields.inventory.state() == SourceValueState::Invalid
        || fields.collection.state() == SourceValueState::Invalid
    {
        return Ok(None);
    }
    let inventory = fields
        .collection
        .value()
        .map_or(&fields.inventory, |collection| &collection.inventory);
    let Some(lines) = inventory.value() else {
        return Ok(None);
    };
    let mut found = BTreeMap::<String, Vec<&FieldOrigin>>::new();
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
            let Some(guid) = item_guid(alternative.value(), parsed)? else {
                dropped += 1;
                continue;
            };
            source::push(found.entry(guid).or_default(), alternative.origin())?;
        }
    }
    let mut guids = Vec::new();
    guids
        .try_reserve_exact(found.len())
        .map_err(CatalogError::JsonSourceAllocation)?;
    for (guid, origins) in found {
        source::check_cancel(cancel)?;
        guids.push(sources.value_with_origins(SourceValue::Present(guid), &origins)?);
    }
    let guids = inventory.map_ref(|_| ShopItemGuids(guids.into()));
    Ok(Some(ItemList::Items { guids, dropped }))
}

fn item_guid(
    reference: Option<&DecimalU64>,
    parsed: &ParsedEntryJson<'_>,
) -> Result<Option<String>, CatalogError> {
    let Some(reference) = reference else {
        return Ok(None);
    };
    let node = parsed.node(source::index(*reference)?)?;
    let SourceNodeValue::String(text) = &node.value else {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    };
    Ok(text
        .text()
        .filter(|text| shop_registry::is_guid(text))
        .map(str::to_ascii_lowercase))
}

pub(super) fn why_unparsed(bytes: &[u8]) -> &'static str {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        "the file is empty"
    } else {
        "the file is no inventory JSON"
    }
}
