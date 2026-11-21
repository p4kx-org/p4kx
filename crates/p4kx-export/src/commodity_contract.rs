// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceUse, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::commodity_authoring_contract::CommodityAuthoring;
use crate::guid_text::CatalogGuid;
use crate::schema_fields::every_field_required;

/// The physical Commodity input and its materialization decision.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CommodityDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) selection: SourcedValue<CommoditySelection>,
    pub(crate) entity: SourcedValue<CommodityAuthoring<'source>>,
}

/// Why this physical Record supplies a Commodity row.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum CommoditySelection {
    Entity,
    ResourceFallback,
}

/// The established Commodity choices independently of their authored input facts.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CommodityPresentation {
    /// The Commodity identity, matching the source GUID in the definitions table.
    pub(crate) commodity_guid: SourcedValue<CatalogGuid>,
    pub(crate) name: SourcedValue<Option<PublicNameRef>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<Option<GameTextRef>>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_type: SourcedValue<Option<GameTextRef>>,
    pub(crate) type_guid: SourcedValue<Option<CatalogGuid>>,
    /// The nullable `ResourceType` link shared by zero, one or several commodities per target.
    pub(crate) subtype_guid: SourcedValue<Option<CatalogGuid>>,
    pub(crate) density_grams_per_cubic_centimetre: SourcedValue<Option<f32>>,
    pub(crate) refined_subtype_guid: SourcedValue<Option<CatalogGuid>>,
}

/// Prepared source facts and choices with internal selection/accounting keys.
#[derive(Debug)]
pub(crate) struct Commodity<'source> {
    pub(crate) guid: CatalogGuid,
    pub(crate) record_path: &'source str,
    pub(crate) subtype_guid: Option<CatalogGuid>,
    pub(crate) definition: CommodityDefinition<'source>,
    pub(crate) presentation: CommodityPresentation,
    pub(crate) relationship_inputs: Vec<SourceUse>,
    pub(crate) crafter_inputs: Vec<SourceUse>,
    pub(crate) crafter_complete: bool,
}

impl<'source> CommodityDefinition<'source> {
    /// Borrows the original physical Record header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Returns whether this row is supplied by an otherwise unrepresented `ResourceType`.
    #[must_use]
    pub fn resource_fallback(&self) -> SourcedValue<bool> {
        self.selection
            .map_ref(|selection| matches!(selection, CommoditySelection::ResourceFallback))
    }

    /// Borrows the original bindings, when the row has a physical Entity.
    #[must_use]
    pub const fn entity(&self) -> &SourcedValue<CommodityAuthoring<'source>> {
        &self.entity
    }
}

impl CommodityPresentation {
    /// Borrows the original Commodity identity.
    #[must_use]
    pub const fn commodity_guid(&self) -> &SourcedValue<CatalogGuid> {
        &self.commodity_guid
    }

    /// Borrows the selected public name.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<Option<PublicNameRef>> {
        &self.name
    }

    /// Borrows the selected protected description.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<Option<GameTextRef>> {
        &self.description
    }

    /// Borrows the selected protected display type.
    #[must_use]
    pub const fn display_type(&self) -> &SourcedValue<Option<GameTextRef>> {
        &self.display_type
    }

    /// Borrows the first directly containing `ResourceTypeGroup`.
    #[must_use]
    pub const fn type_guid(&self) -> &SourcedValue<Option<CatalogGuid>> {
        &self.type_guid
    }

    /// Borrows the nullable and non-unique `ResourceType` relationship.
    #[must_use]
    pub const fn subtype_guid(&self) -> &SourcedValue<Option<CatalogGuid>> {
        &self.subtype_guid
    }

    /// Borrows the finite density in grams per cubic centimetre.
    #[must_use]
    pub const fn density_grams_per_cubic_centimetre(&self) -> &SourcedValue<Option<f32>> {
        &self.density_grams_per_cubic_centimetre
    }

    /// Borrows the resolved refined `ResourceType` relationship.
    #[must_use]
    pub const fn refined_subtype_guid(&self) -> &SourcedValue<Option<CatalogGuid>> {
        &self.refined_subtype_guid
    }
}
