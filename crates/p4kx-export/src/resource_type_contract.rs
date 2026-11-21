// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_extraction::SourcedValue;
use schemars::{JsonSchema, Schema, SchemaGenerator};
use serde::Serialize;

use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_text_contract::{GameTextRef, PublicNameRef};
use crate::guid_text::CatalogGuid;
use crate::resource_density_source::ResourceDensitySource;
use crate::resource_property_contract::{ResourceCargoSlot, ResourceProperty};
use crate::schema_fields::every_field_required;

/// A selected resource definition with independent internal selection identity.
#[derive(Debug, Serialize)]
pub(crate) struct ResourceType<'source> {
    #[serde(skip)]
    pub(crate) guid: CatalogGuid,
    #[serde(skip)]
    pub(crate) record_path: &'source str,
    #[serde(skip)]
    pub(crate) numbers: Vec<CapturedNumber>,
    #[serde(flatten)]
    pub(crate) data: ResourceDefinition<'source>,
}

impl JsonSchema for ResourceType<'_> {
    fn schema_name() -> Cow<'static, str> {
        "ResourceType".into()
    }

    fn json_schema(generator: &mut SchemaGenerator) -> Schema {
        ResourceDefinition::json_schema(generator)
    }
}

/// Canonical resource data values with immediate source provenance.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ResourceDefinition<'source> {
    #[serde(flatten)]
    pub(crate) source: CanonicalRecordHeader<'source>,
    pub(crate) name: SourcedValue<PublicNameRef>,
    #[schemars(extend("x-gameText" = true))]
    pub(crate) description: SourcedValue<GameTextRef>,
    pub(crate) group_guids: SourcedValue<Vec<SourcedValue<CatalogGuid>>>,
    pub(crate) refined_version: SourcedValue<CatalogRecordReference>,
    pub(crate) density: SourcedValue<ResourceDensitySource<'source>>,
    pub(crate) default_thumbnail_path: SourcedValue<&'source str>,
    #[serde(rename = "defaultThumbnailPathSVG")]
    pub(crate) default_thumbnail_path_svg: SourcedValue<&'source str>,
    pub(crate) rtt_thumbnail_entity_class: SourcedValue<CatalogRecordReference>,
    pub(crate) validate_default_cargo_box: SourcedValue<bool>,
    pub(crate) default_cargo_containers:
        SourcedValue<Vec<SourcedValue<ResourceCargoSlot<'source>>>>,
    pub(crate) properties: SourcedValue<Vec<SourcedValue<ResourceProperty<'source>>>>,
}

impl<'source> ResourceDefinition<'source> {
    /// Borrows the original physical Record header and its evidence.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the canonical public-name reference.
    #[must_use]
    pub const fn name(&self) -> &SourcedValue<PublicNameRef> {
        &self.name
    }

    /// Borrows the protected description reference.
    #[must_use]
    pub const fn description(&self) -> &SourcedValue<GameTextRef> {
        &self.description
    }

    /// Borrows all authored group membership references and their states.
    #[must_use]
    pub const fn group_guids(&self) -> &SourcedValue<Vec<SourcedValue<CatalogGuid>>> {
        &self.group_guids
    }

    /// Borrows the original refinement target without following its body.
    #[must_use]
    pub const fn refined_version(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.refined_version
    }

    /// Borrows the authored density or molar-mass source.
    #[must_use]
    pub const fn density(&self) -> &SourcedValue<ResourceDensitySource<'source>> {
        &self.density
    }

    /// Borrows the original thumbnail asset path.
    #[must_use]
    pub const fn default_thumbnail_path(&self) -> &SourcedValue<&'source str> {
        &self.default_thumbnail_path
    }

    /// Borrows the original SVG thumbnail asset path.
    #[must_use]
    pub const fn default_thumbnail_path_svg(&self) -> &SourcedValue<&'source str> {
        &self.default_thumbnail_path_svg
    }

    /// Borrows the original thumbnail Entity reference.
    #[must_use]
    pub const fn rtt_thumbnail_entity_class(&self) -> &SourcedValue<CatalogRecordReference> {
        &self.rtt_thumbnail_entity_class
    }

    /// Borrows the original cargo validation flag without defaulting missing values.
    #[must_use]
    pub const fn validate_default_cargo_box(&self) -> &SourcedValue<bool> {
        &self.validate_default_cargo_box
    }

    /// Borrows the ordered original cargo slots, including unavailable positions.
    #[must_use]
    pub const fn default_cargo_containers(
        &self,
    ) -> &SourcedValue<Vec<SourcedValue<ResourceCargoSlot<'source>>>> {
        &self.default_cargo_containers
    }

    /// Borrows every ordered resource property and its immediate source state.
    #[must_use]
    pub const fn properties(&self) -> &SourcedValue<Vec<SourcedValue<ResourceProperty<'source>>>> {
        &self.properties
    }
}
