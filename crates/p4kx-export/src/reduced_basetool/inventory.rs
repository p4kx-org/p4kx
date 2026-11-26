// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DecimalU64, SourcedValue};
use serde::Serialize;
use serde_json::value::RawValue;

use super::ReducedError;
use super::cell::Cell;
use crate::catalog_queries::shops::{
    InventoryLine, InventoryLines, ShopInventoryDefinition, ShopInventoryDocument,
    ShopInventorySource, ShopInventoryString, ShopInventoryToken,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Inventory<'source> {
    entry_index: u32,
    source_path: Cell<&'source String>,
    normalized_path: Cell<&'source String>,
    document: Cell<Document<'source>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Document<'source> {
    shop_id: Cell<&'source ShopInventoryString>,
    inventory: Cell<Vec<Cell<Line<'source>>>>,
    collection: Cell<Collection<'source>>,
}

#[derive(Serialize)]
struct Collection<'source> {
    inventory: Cell<Vec<Cell<Line<'source>>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Line<'source> {
    id: Cell<Identifier<'source>>,
    buy_price: Cell<&'source RawValue>,
    sell_price: Cell<&'source RawValue>,
    current_inventory: Cell<&'source RawValue>,
    max_inventory: Cell<&'source RawValue>,
}

#[derive(Serialize)]
struct Identifier<'source> {
    alternatives: Cell<Vec<Cell<&'source ShopInventoryString>>>,
}

impl<'source> Inventory<'source> {
    pub(super) fn project(
        definition: &'source ShopInventoryDefinition,
    ) -> Result<Self, ReducedError> {
        let identity = definition.identity();
        let entry_index = identity.entry_index();
        let document = Cell::from(definition.document()).try_map(Document::project)?;
        if document.value.is_none() {
            return Err(ReducedError::UnreadableInventory(entry_index));
        }
        Ok(Self {
            entry_index,
            source_path: identity.source_path().into(),
            normalized_path: identity.normalized_path().into(),
            document,
        })
    }
}

impl<'source> Document<'source> {
    fn project(document: &'source ShopInventoryDocument) -> Result<Self, ReducedError> {
        let source = document.source();
        let fields = document.fields();
        Ok(Self {
            shop_id: text(source, fields.shop_id())?,
            inventory: lines(source, fields.inventory())?,
            collection: Cell::from(fields.collection()).try_map(|collection| {
                Ok::<_, ReducedError>(Collection {
                    inventory: lines(source, collection.inventory())?,
                })
            })?,
        })
    }
}

fn lines<'source>(
    source: &'source ShopInventorySource,
    field: &'source SourcedValue<InventoryLines>,
) -> Result<Cell<Vec<Cell<Line<'source>>>>, ReducedError> {
    Cell::from(field).try_map(|lines| {
        lines
            .iter()
            .map(|line| Cell::from(line).try_map(|line| project_line(source, line)))
            .collect()
    })
}

fn project_line<'source>(
    source: &'source ShopInventorySource,
    line: &'source InventoryLine,
) -> Result<Line<'source>, ReducedError> {
    Ok(Line {
        id: Cell::from(line.id()).try_map(|id| {
            let alternatives = Cell::from(id.alternatives()).try_map(|cells| {
                cells
                    .iter()
                    .map(|field| text(source, field))
                    .collect::<Result<_, _>>()
            })?;
            Ok::<_, ReducedError>(Identifier { alternatives })
        })?,
        buy_price: number(source, line.buy_price())?,
        sell_price: number(source, line.sell_price())?,
        current_inventory: number(source, line.current_inventory())?,
        max_inventory: number(source, line.max_inventory())?,
    })
}

fn text<'source>(
    source: &'source ShopInventorySource,
    field: &'source SourcedValue<DecimalU64>,
) -> Result<Cell<&'source ShopInventoryString>, ReducedError> {
    dereference(source, field, |token| match token {
        ShopInventoryToken::String { value } => Ok(value),
        _ => Err(ReducedError::InvalidInventoryReference),
    })
}

fn number<'source>(
    source: &'source ShopInventorySource,
    field: &'source SourcedValue<DecimalU64>,
) -> Result<Cell<&'source RawValue>, ReducedError> {
    dereference(source, field, |token| match token {
        ShopInventoryToken::Number { token } => Ok(serde_json::from_str(token)?),
        _ => Err(ReducedError::InvalidInventoryReference),
    })
}

fn dereference<'source, T>(
    source: &'source ShopInventorySource,
    field: &'source SourcedValue<DecimalU64>,
    decode: impl FnOnce(&'source ShopInventoryToken) -> Result<T, ReducedError>,
) -> Result<Cell<T>, ReducedError> {
    let selector = Cell::from(field);
    let Some(index) = selector.value else {
        return Ok(Cell {
            state: selector.state,
            value: None,
            contains_encrypted_source: selector.contains_encrypted_source,
        });
    };
    let address =
        usize::try_from(index.get()).map_err(|_| ReducedError::InvalidInventoryReference)?;
    let target = source
        .values()
        .get(address)
        .ok_or(ReducedError::InvalidInventoryReference)?;
    let value = target
        .value()
        .ok_or(ReducedError::InvalidInventoryReference)?;
    let marker = match (
        selector.contains_encrypted_source,
        target.origin().contains_encrypted_source(),
    ) {
        (Some(true), _) | (_, Some(true)) => Some(true),
        (Some(false), Some(false)) => Some(false),
        _ => None,
    };
    Ok(Cell {
        state: selector.state,
        value: Some(decode(value)?),
        contains_encrypted_source: marker,
    })
}
