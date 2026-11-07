// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::StructValue;

use crate::resource_quality_read;
use crate::resource_type_contract::{
    ResourceCraftingData, ResourceProperty, ResourcePropertyPayload, ResourceVolatility,
};
use crate::source_fields::{FieldError, Fields};

/// Projects every authored resource-property subtype without deduplication.
pub(crate) fn read<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceProperty<'source>, FieldError> {
    let fields = Fields::new(source, path);
    let name = fields.text("name")?;
    let payload = match source.type_name() {
        "ResourceTypeProperties" => {
            fields.check(&["name"], notes);
            ResourcePropertyPayload::Base
        }
        "ResourceTypeVolatility" => {
            fields.check(&["name", "volatility", "healthDecayPerSecond"], notes);
            ResourcePropertyPayload::Volatility(ResourceVolatility {
                volatility: fields.single("volatility")?,
                health_decay_per_second: fields.single("healthDecayPerSecond")?,
            })
        }
        "ResourceTypeCraftingData" => {
            ResourcePropertyPayload::CraftingData(Box::new(crafting(source, path, notes)?))
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourcePropertyPayload::Unsupported
        }
    };
    Ok(ResourceProperty {
        source_type: source.type_name(),
        name,
        payload,
    })
}

fn crafting<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceCraftingData<'source>, FieldError> {
    let fields = Fields::new(source, path);
    fields.check(
        &[
            "name",
            "qualityDistribution",
            "qualityLocationOverride",
            "qualityQuantization",
        ],
        notes,
    );
    let quality_distribution = fields
        .structure("qualityDistribution")?
        .map(|value| {
            resource_quality_read::distribution(
                value,
                &format!("{path}.qualityDistribution"),
                notes,
            )
        })
        .transpose()?;
    let quality_location_override = fields
        .structure("qualityLocationOverride")?
        .map(|value| {
            resource_quality_read::overrides(
                value,
                &format!("{path}.qualityLocationOverride"),
                notes,
            )
        })
        .transpose()?;
    let quality_quantization = fields
        .structure("qualityQuantization")?
        .map(|value| {
            resource_quality_read::quantization(
                value,
                &format!("{path}.qualityQuantization"),
                notes,
            )
        })
        .transpose()?;
    Ok(ResourceCraftingData {
        distribution: quality_distribution,
        location_override: quality_location_override,
        quantization: quality_quantization,
    })
}
