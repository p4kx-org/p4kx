// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::blueprint_definition::PhysicalBlueprintRef;
use crate::catalog_text_contract::GameTextRef;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;
use crate::source_value_catalog::SourceValueRef;

/// Per-Blueprint lookups and first-tier convenience navigation.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintPresentation<Category> {
    pub(crate) blueprint: PhysicalBlueprintRef,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) name: SourcedValue<GameTextRef>,
    pub(crate) category: Category,
    pub(crate) produced_item: SourcedValue<CatalogGuid>,
    pub(crate) first_tier: Option<SourceValueRef>,
    pub(crate) ingredients: Vec<BlueprintIngredient>,
    pub(crate) default_available: SourcedValue<bool>,
    pub(crate) dismantle: SourcedValue<BlueprintDismantle>,
}

/// The first-tier ingredient and its nearest enclosing named selection.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintIngredient {
    pub(crate) cost: SourceValueRef,
    pub(crate) slot: Option<SourceValueRef>,
}

/// The selected generic process and the tier whose costs it returns.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct BlueprintDismantle {
    pub(crate) process_guid: CatalogGuid,
    pub(crate) tier: Option<SourceValueRef>,
}

impl<Category> BlueprintPresentation<Category> {
    /// Borrows the physical Blueprint selected by this presentation.
    #[must_use]
    pub const fn blueprint(&self) -> &PhysicalBlueprintRef {
        &self.blueprint
    }

    /// Borrows the protected localized Blueprint name identity.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<GameTextRef> {
        &self.name
    }

    /// Borrows the interpreted category selection.
    #[must_use]
    pub const fn category(&self) -> &Category {
        &self.category
    }

    /// Borrows the selected produced item GUID.
    #[must_use]
    pub const fn produced_item(&self) -> &SourcedValue<CatalogGuid> {
        &self.produced_item
    }

    /// Returns the first interpreted original tier occurrence.
    #[must_use]
    pub const fn first_tier(&self) -> Option<SourceValueRef> {
        self.first_tier
    }

    /// Borrows first-tier ingredient navigation in authored order.
    #[must_use]
    pub fn ingredients(&self) -> &[BlueprintIngredient] {
        &self.ingredients
    }

    /// Borrows the authored default-availability selection result.
    #[must_use]
    pub const fn default_available(&self) -> &SourcedValue<bool> {
        &self.default_available
    }

    /// Borrows the selected dismantle process and returned cost tier.
    #[must_use]
    pub const fn dismantle(&self) -> &SourcedValue<BlueprintDismantle> {
        &self.dismantle
    }
}

impl BlueprintIngredient {
    /// Returns the original first-tier ingredient occurrence.
    #[must_use]
    pub const fn cost(&self) -> SourceValueRef {
        self.cost
    }

    /// Returns the nearest enclosing named selection occurrence.
    #[must_use]
    pub const fn slot(&self) -> Option<SourceValueRef> {
        self.slot
    }
}

impl BlueprintDismantle {
    /// Returns the selected generic dismantle process GUID.
    #[must_use]
    pub const fn process_guid(&self) -> CatalogGuid {
        self.process_guid
    }

    /// Returns the original tier whose costs the process returns.
    #[must_use]
    pub const fn tier(&self) -> Option<SourceValueRef> {
        self.tier
    }
}
