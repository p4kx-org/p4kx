// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue};

use crate::CatalogError;
use crate::resource_property_contract::{
    ResourceCraftingData, ResourceProperty, ResourcePropertyPayload, ResourceVolatility,
};
use crate::resource_quality_read;
use crate::resource_value_read::{ResourceContext, ResourceFields};
use crate::source_fields::FieldError;

/// Projects a concrete property without collapsing nulls or reordering members.
pub(crate) fn read<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &ResourceContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceProperty<'f>, CatalogError> {
    let fields = ResourceFields::new(source, path, context);
    let name = fields.text("name", notes)?;
    let payload = match source.type_name() {
        "ResourceTypeProperties" => {
            fields.check(&["name"], notes);
            Some(ResourcePropertyPayload::Base)
        }
        "ResourceTypeVolatility" => {
            fields.check(&["name", "volatility", "healthDecayPerSecond"], notes);
            Some(ResourcePropertyPayload::Volatility(ResourceVolatility {
                volatility: fields.single("volatility", notes)?,
                health_decay_per_second: fields.single("healthDecayPerSecond", notes)?,
            }))
        }
        "ResourceTypeCraftingData" => Some(ResourcePropertyPayload::CraftingData(Box::new(
            crafting(source, path, context, notes)?,
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
    context: &ResourceContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceCraftingData<'f>, CatalogError> {
    let fields = ResourceFields::new(source, path, context);
    fields.check(
        &[
            "name",
            "qualityDistribution",
            "qualityLocationOverride",
            "qualityQuantization",
        ],
        notes,
    );
    let distribution = fields
        .structure("qualityDistribution", notes)?
        .try_map(|value| {
            resource_quality_read::distribution(
                value,
                &format!("{path}.qualityDistribution"),
                context,
                notes,
            )
        })?;
    let location_override = fields
        .structure("qualityLocationOverride", notes)?
        .try_map(|value| {
            resource_quality_read::overrides(
                value,
                &format!("{path}.qualityLocationOverride"),
                context,
                notes,
            )
        })?;
    let quantization = fields
        .structure("qualityQuantization", notes)?
        .try_map(|value| {
            resource_quality_read::quantization(
                value,
                &format!("{path}.qualityQuantization"),
                context,
                notes,
            )
        })?;
    Ok(ResourceCraftingData {
        distribution,
        location_override,
        quantization,
    })
}
