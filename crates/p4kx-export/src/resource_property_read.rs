// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::quality_source_read;
use crate::resource_property_contract::{
    ResourceCraftingData, ResourceProperty, ResourcePropertyPayload, ResourceVolatility,
};
use crate::resource_value_read;
use crate::source_fields::{FieldCapture, FieldError};

/// Projects a concrete property without collapsing nulls or reordering members.
pub(crate) fn read<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<ResourceProperty<'f>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let name = fields.text("name", notes)?;
    let payload = match source.type_name() {
        "ResourceTypeProperties" => {
            fields.check_observed(&["name"], notes)?;
            Some(ResourcePropertyPayload::Base)
        }
        "ResourceTypeVolatility" => {
            fields.check_observed(&["name", "volatility", "healthDecayPerSecond"], notes)?;
            Some(ResourcePropertyPayload::Volatility(ResourceVolatility {
                volatility: resource_value_read::single(&fields, "volatility", notes)?,
                health_decay_per_second: resource_value_read::single(
                    &fields,
                    "healthDecayPerSecond",
                    notes,
                )?,
            }))
        }
        "ResourceTypeCraftingData" => Some(ResourcePropertyPayload::CraftingData(Box::new(
            crafting(source, path, context, notes, capture)?,
        ))),
        _ => None,
    };
    Ok(ResourceProperty {
        source_type: fields.source_type()?,
        name,
        payload: payload.map_or_else(
            || fields.unsupported(notes),
            |payload| context.selector(&format!("{path}.$type"), SourceValue::Present(payload)),
        )?,
    })
}

fn crafting<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<ResourceCraftingData<'f>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    fields.check_observed(
        &[
            "name",
            "qualityDistribution",
            "qualityLocationOverride",
            "qualityQuantization",
        ],
        notes,
    )?;
    let distribution = fields
        .structure("qualityDistribution", notes)?
        .try_map(|value| {
            quality_source_read::distribution(
                value,
                &format!("{path}.qualityDistribution"),
                context,
                notes,
                capture,
            )
        })?;
    let location_override = fields
        .structure("qualityLocationOverride", notes)?
        .try_map(|value| {
            quality_source_read::overrides(
                value,
                &format!("{path}.qualityLocationOverride"),
                context,
                notes,
                capture,
            )
        })?;
    let quantization = fields
        .structure("qualityQuantization", notes)?
        .try_map(|value| {
            quality_source_read::quantization(
                value,
                &format!("{path}.qualityQuantization"),
                context,
                notes,
                capture,
            )
        })?;
    Ok(ResourceCraftingData {
        distribution,
        location_override,
        quantization,
    })
}
