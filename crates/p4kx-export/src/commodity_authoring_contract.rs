// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::schema_fields::every_field_required;

/// Ordered commodity-owned bindings without presentation fallback.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CommodityAuthoring<'source> {
    pub(crate) purchasable_parameters:
        SourcedValue<Vec<SourcedValue<CommodityPurchasable<'source>>>>,
    pub(crate) display_parameters: SourcedValue<Vec<SourcedValue<CommodityDisplay<'source>>>>,
    pub(crate) resource_containers:
        SourcedValue<Vec<SourcedValue<CommodityContainerLink<'source>>>>,
}

/// One original purchasable component with independently sourced text declarations.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CommodityPurchasable<'source> {
    pub(crate) source_position: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
    pub(crate) display_name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_type: SourcedValue<GameTextRef>,
}

/// One original display component with its own description declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CommodityDisplay<'source> {
    pub(crate) source_position: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) display_description: SourcedValue<GameTextRef>,
}

/// A physical link to the `ResourceContainer` configuration of this Commodity Entity.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct CommodityContainerLink<'source> {
    pub(crate) owner_record_index: SourcedValue<u32>,
    pub(crate) source_position: SourcedValue<u32>,
    pub(crate) source_type: SourcedValue<&'source str>,
}

impl<'source> CommodityAuthoring<'source> {
    /// Borrows authored purchaser bindings in source order.
    #[must_use]
    pub const fn purchasable_parameters(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<CommodityPurchasable<'source>>>> {
        &self.purchasable_parameters
    }

    /// Borrows authored display bindings in source order.
    #[must_use]
    pub const fn display_parameters(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<CommodityDisplay<'source>>>> {
        &self.display_parameters
    }

    /// Borrows authored `ResourceContainer` bindings in source order.
    #[must_use]
    pub const fn resource_containers(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<CommodityContainerLink<'source>>>> {
        &self.resource_containers
    }
}

impl<'source> CommodityPurchasable<'source> {
    /// Borrows the original component ordinal.
    #[must_use]
    pub const fn source_position(&self) -> &SourcedValue<u32> {
        &self.source_position
    }

    /// Borrows the original component type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored public name.
    #[must_use]
    pub const fn display_name(&self) -> &SourcedValue<PublicNameRef> {
        &self.display_name
    }

    /// Borrows the authored protected type text.
    #[must_use]
    pub const fn display_type(&self) -> &SourcedValue<GameTextRef> {
        &self.display_type
    }
}

impl<'source> CommodityDisplay<'source> {
    /// Borrows the original component ordinal.
    #[must_use]
    pub const fn source_position(&self) -> &SourcedValue<u32> {
        &self.source_position
    }

    /// Borrows the original component type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored protected description.
    #[must_use]
    pub const fn display_description(&self) -> &SourcedValue<GameTextRef> {
        &self.display_description
    }
}

impl<'source> CommodityContainerLink<'source> {
    /// Borrows the physical Entity Record index.
    #[must_use]
    pub const fn owner_record_index(&self) -> &SourcedValue<u32> {
        &self.owner_record_index
    }

    /// Borrows the original component ordinal.
    #[must_use]
    pub const fn source_position(&self) -> &SourcedValue<u32> {
        &self.source_position
    }

    /// Borrows the original component type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }
}
