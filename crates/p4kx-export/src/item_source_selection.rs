// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceValue, SourcedValue, Value};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::item_component_owner::ComponentOwner;
use super::item_attachment;
use super::item_definition::PURCHASABLE;
use crate::CatalogError;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_fields;
use crate::schema_fields::every_field_required;

/// The first physical presentation components and the source prefixes selecting them.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct ItemSourceSelection {
    #[serde(rename = "attachmentComponentOrdinal")]
    attachment: SourcedValue<u32>,
    #[serde(rename = "physicsComponentOrdinal")]
    physics: SourcedValue<u32>,
    #[serde(rename = "purchasableComponentOrdinal")]
    purchasable: SourcedValue<u32>,
}

impl ItemSourceSelection {
    pub(crate) fn read<'source>(
        tree: &RecordTree<'source>,
        originals: &OriginalFlowFields<'_, '_, 'source>,
    ) -> Result<Self, CatalogError> {
        let select_component = |source_type| {
            let selected = record_fields::component_occurrence(tree, source_type);
            select(originals, selected.map(|(value, _)| value))
        };
        Ok(Self {
            attachment: Self::read_attachment(tree, originals)?,
            physics: select_component("SEntityPhysicsControllerParams")?,
            purchasable: Self::with_purchasable(tree, originals, |ordinal, _| Ok(ordinal))?,
        })
    }

    pub(crate) fn with_purchasable<'read, 'source, T>(
        tree: &'read RecordTree<'source>,
        originals: &OriginalFlowFields<'_, '_, 'source>,
        inspect: impl FnOnce(
            SourcedValue<u32>,
            Option<&'read Value<'source>>,
        ) -> Result<T, CatalogError>,
    ) -> Result<T, CatalogError> {
        let occurrence =
            record_fields::component_occurrence(tree, PURCHASABLE).map(|(value, _)| value);
        inspect(select(originals, occurrence)?, occurrence)
    }

    pub(crate) fn read_attachment<'source>(
        tree: &RecordTree<'source>,
        originals: &OriginalFlowFields<'_, '_, 'source>,
    ) -> Result<SourcedValue<u32>, CatalogError> {
        select(
            originals,
            item_attachment::first(tree).map(|(value, _)| value),
        )
    }

    /// Borrows the first physical attachment component ordinal.
    #[must_use]
    pub const fn attachment_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.attachment
    }

    /// Borrows the first physical physics component ordinal.
    #[must_use]
    pub const fn physics_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.physics
    }

    /// Borrows the first physical purchasable component ordinal.
    #[must_use]
    pub const fn purchasable_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.purchasable
    }
}

fn select<'source>(
    originals: &OriginalFlowFields<'_, '_, 'source>,
    selected: Option<&Value<'source>>,
) -> Result<SourcedValue<u32>, CatalogError> {
    let prefix = originals.component_selection(selected)?;
    let uses = originals.sources();
    let Some(value) = selected else {
        let state = if prefix.complete {
            SourceValue::Absent
        } else {
            SourceValue::Unreadable
        };
        return Ok(uses.value(state, &prefix.uses, prefix.complete));
    };
    let value = ComponentOwner::ordinal(originals, originals.fact(value)?)?;
    let placement = uses.origin(&prefix.uses, prefix.complete);
    Ok(uses.add_value_origins(value, &[&placement])?)
}
