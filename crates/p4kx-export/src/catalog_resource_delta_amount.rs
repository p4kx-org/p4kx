// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_resource_flow_unit::ResourceFlowUnit;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const RESOURCE_ENUM: &str = "ResourceNetworkResource";

/// One side of an original resource delta.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceDeltaAmount<'source> {
    source_type: SourcedValue<&'source str>,
    resource: SourcedValue<&'source str>,
    resource_amount_per_second: SourcedValue<ResourceFlowUnit<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceDeltaAmount<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let resource = source.borrowed_enum_or_string("resource", RESOURCE_ENUM, notes)?;
        let resource_amount_per_second =
            source.declared_pointer("resourceAmountPerSecond", notes, ResourceFlowUnit::read)?;
        let additional_fields = source.authored_members_except([
            ("resource", resource.state()),
            (
                "resourceAmountPerSecond",
                resource_amount_per_second.state(),
            ),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            resource,
            resource_amount_per_second,
            additional_fields,
        })
    }
}

/// Ordered authored resource ratios attached to a delta side.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceComposition<'source> {
    source_type: SourcedValue<&'source str>,
    values: SourcedValue<Vec<SourcedValue<CompositionValue<'source>>>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ResourceComposition<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let values = source.declared_class_array("values", notes, CompositionValue::read)?;
        let additional_fields = source.authored_members_except([("values", values.state())])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            values,
            additional_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct CompositionValue<'source> {
    source_type: SourcedValue<&'source str>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    container_resource: SourcedValue<Box<SourceRecordReference<'source>>>,
    ratio: SourcedValue<ResourceFloat32>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> CompositionValue<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let container_resource =
            source.record_link_to("containerResource", "ResourceType", notes)?;
        let ratio = source.single("ratio", notes)?;
        let additional_fields = source.authored_members_except([
            ("containerResource", container_resource.state()),
            ("ratio", ratio.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            container_resource,
            ratio,
            additional_fields,
        })
    }
}
