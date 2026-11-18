// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::paint_facts::ItemPaintFacts;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::catalog_item_components::item_definition::ItemDefinition;
use crate::catalog_v3::catalog_item_components::selection::ItemSourceSelection;
use crate::contract::every_field_required;
use crate::guid_text::CatalogGuid;
use crate::item_variant::ItemKey;
use crate::{CatalogError, ExportAccountingError, ItemClass, Playability};

/// The selected physical Item and its source-backed classification projections.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Item<'source> {
    #[serde(flatten)]
    pub(crate) header: CanonicalRecordHeader<'source>,
    pub(crate) selection: ItemSourceSelection,
    #[serde(rename = "itemClass")]
    #[schemars(with = "SourcedValue<String>")]
    pub(crate) class: SourcedValue<ItemClass>,
    #[schemars(with = "SourcedValue<String>")]
    pub(crate) playability: SourcedValue<Playability>,
    pub(crate) variant_of: Option<CatalogGuid>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) identity: ItemIdentity<'source>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) paint: ItemPaintFacts<'source>,
    #[serde(skip)]
    #[schemars(skip)]
    pub(crate) variant_key: Option<ItemKey<'source>>,
}

/// Compact identity keys taken from the same admitted canonical header.
#[derive(Debug)]
pub(crate) struct ItemIdentity<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) class_name: &'source str,
}

impl<'source> ItemIdentity<'source> {
    pub(super) fn read(header: &CanonicalRecordHeader<'source>) -> Result<Self, CatalogError> {
        let guid = header
            .guid
            .value()
            .copied()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        let class_name = header
            .class_name
            .value()
            .copied()
            .ok_or(ExportAccountingError::MissingFieldInspection)?;
        Ok(Self { guid, class_name })
    }
}

pub(super) fn check_source(
    tree: &RecordTree<'_>,
    definition: &ItemDefinition<'_>,
    projection: &CatalogProjection<'_, '_>,
) -> Result<(), CatalogError> {
    let header = definition.source();
    if header.record_index.value().copied() != Some(tree.record_index())
        || header.guid.value().map(|guid| guid.0) != Some(tree.guid())
    {
        return Err(ExportAccountingError::ConflictingSourceDatum.into());
    }
    projection
        .origins()
        .uses
        .origin_with_origins([header.guid.origin(), header.record_index.origin()], true)?;
    Ok(())
}
