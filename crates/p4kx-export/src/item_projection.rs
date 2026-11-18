// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceValueState, SourcedValue};
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

/// Source-backed fields of one selected physical Item.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemSource<'source> {
    #[serde(flatten)]
    pub(super) header: CanonicalRecordHeader<'source>,
    pub(super) selection: ItemSourceSelection,
    #[serde(rename = "itemClass")]
    #[schemars(with = "SourcedValue<String>")]
    pub(super) class: SourcedValue<ItemClass>,
    #[schemars(with = "SourcedValue<String>")]
    pub(super) playability: SourcedValue<Playability>,
}

impl<'source> ItemSource<'source> {
    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.header
    }

    /// Borrows the first physical positions of the presentation components.
    #[must_use]
    pub const fn selection(&self) -> &ItemSourceSelection {
        &self.selection
    }

    /// Borrows the classification from the original English Description lookup.
    #[must_use]
    pub const fn item_class(&self) -> &SourcedValue<ItemClass> {
        &self.class
    }

    /// Borrows the playability derived from original keys and Record path.
    #[must_use]
    pub const fn playability(&self) -> &SourcedValue<Playability> {
        &self.playability
    }

    pub(crate) fn source_set_complete(&self) -> bool {
        complete(self.selection.attachment_component_ordinal())
            && complete(self.selection.physics_component_ordinal())
            && complete(self.selection.purchasable_component_ordinal())
            && complete(&self.class)
            && complete(&self.playability)
    }
}

fn complete<T>(value: &SourcedValue<T>) -> bool {
    matches!(
        value.state(),
        SourceValueState::Present | SourceValueState::Absent | SourceValueState::Null
    ) && value.origin().source_set_complete()
}

/// One physical Item with its source fields and variant decision.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Item<'source> {
    #[serde(flatten)]
    pub(crate) source: ItemSource<'source>,
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
