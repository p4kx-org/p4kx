// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::blueprint_recipe_coordinates::PATH;
use super::cell::Cell;
use super::completeness;
use crate::CatalogError;
use crate::blueprint_context_contract::{
    BlueprintCostContext, BlueprintModifier, BlueprintModifierRange, BlueprintRangeKind,
};
use crate::catalog_query_session::checkpoint;
use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;
use crate::source_value_catalog::{SourceArenaOutput, SourcePayload, SourceText, SourceValueRef};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Context<'read, 'source> {
    quantity_multiplier: Option<Cell<&'read f32>>,
    composition_inclusion: Option<Cell<&'read SourceText<'source>>>,
    modifiers: Vec<Modifier<'read>>,
}

impl<'read, 'source> Context<'read, 'source> {
    pub(super) fn read(
        source: &'read BlueprintCostContext,
        original: &'read SourceArenaOutput<'source>,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ReducedError> {
        let mut modifiers = Vec::new();
        modifiers
            .try_reserve_exact(source.modifiers().len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for modifier in source.modifiers() {
            modifiers.push(Modifier::read(modifier, cancel)?);
        }
        Ok(Self {
            quantity_multiplier: source
                .quantity_multiplier()
                .map(|value| completeness::cell(PATH, value))
                .transpose()?,
            composition_inclusion: text(original, source.composition_inclusion())?,
            modifiers,
        })
    }
}

pub(super) fn text<'read, 'source>(
    original: &'read SourceArenaOutput<'source>,
    reference: Option<SourceValueRef>,
) -> Result<Option<Cell<&'read SourceText<'source>>>, ReducedError> {
    reference
        .map(|reference| {
            let source = original
                .value(reference)
                .ok_or(ReducedError::IncompleteScope(PATH))?;
            completeness::cell(PATH, source.value())?.try_map(|value| match value {
                SourcePayload::String { value } | SourcePayload::Locale { value } => Ok(value),
                _ => Err(ReducedError::IncompleteScope(PATH)),
            })
        })
        .transpose()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Modifier<'read> {
    property_guid: Cell<&'read CatalogGuid>,
    ranges: Vec<Range<'read>>,
}

impl<'read> Modifier<'read> {
    fn read(
        source: &'read BlueprintModifier,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, ReducedError> {
        checkpoint(cancel)?;
        let mut ranges = Vec::new();
        ranges
            .try_reserve_exact(source.ranges().len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for range in source.ranges() {
            checkpoint(cancel)?;
            ranges.push(Range::read(range)?);
        }
        Ok(Self {
            property_guid: completeness::cell(PATH, source.property_guid())?,
            ranges,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Range<'read> {
    kind: &'read BlueprintRangeKind,
    start_quality: Cell<&'read ResourceInt64>,
    end_quality: Cell<&'read ResourceInt64>,
    at_start: Cell<&'read f32>,
    at_end: Cell<&'read f32>,
}

impl<'read> Range<'read> {
    fn read(source: &'read BlueprintModifierRange) -> Result<Self, ReducedError> {
        Ok(Self {
            kind: source.kind(),
            start_quality: completeness::cell(PATH, source.start_quality())?,
            end_quality: completeness::cell(PATH, source.end_quality())?,
            at_start: completeness::cell(PATH, source.at_start())?,
            at_end: completeness::cell(PATH, source.at_end())?,
        })
    }
}
