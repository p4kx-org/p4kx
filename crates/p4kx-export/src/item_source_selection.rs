// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{RecordTree, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_read::original::OriginalFlowFields;
use crate::record_fields;
use crate::schema_fields::every_field_required;

/// The first physical presentation components and the source prefixes selecting them.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub(crate) struct ItemSourceSelection {
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
        Ok(Self {
            attachment: select(tree, originals, "SAttachableComponentParams")?,
            physics: select(tree, originals, "SEntityPhysicsControllerParams")?,
            purchasable: select(tree, originals, "SCItemPurchasableParams")?,
        })
    }

    pub(crate) const fn attachment_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.attachment
    }

    pub(crate) const fn purchasable_component_ordinal(&self) -> &SourcedValue<u32> {
        &self.purchasable
    }
}

fn select<'source>(
    tree: &RecordTree<'source>,
    originals: &OriginalFlowFields<'_, '_, 'source>,
    source_type: &str,
) -> Result<SourcedValue<u32>, CatalogError> {
    let selected = record_fields::component_occurrence(tree, source_type).map(|(value, _)| value);
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
