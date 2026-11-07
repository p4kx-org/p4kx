// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{StructValue, Value};

use crate::contract_quantization::QuantizationBand;
use crate::contract_rewards::QualityCurve;
use crate::quality_curve_read;
use crate::resource_quality_source::{
    InlineQuality, QualityRecordReference, ResourceQualityOverride, ResourceQualityOverrideFields,
    ResourceQualityOverridePayload, ResourceQualityOverrides, ResourceQualityPayload,
    ResourceQualityQuantization, ResourceQualitySource,
};
use crate::source_fields::{self, FieldError, Fields};

/// Projects a distribution's authored inline or Record-linked source.
pub(crate) fn distribution<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualitySource<'source, QualityCurve<'source>>, FieldError> {
    let fields = Fields::new(source, path);
    let payload = match source.type_name() {
        "CraftingQualityDistributionNormal" | "CraftingQualityDistributionUniform" => {
            ResourceQualityPayload::Inline(InlineQuality {
                value: quality_curve_read::read(source, path, notes)?,
            })
        }
        "CraftingQualityDistribution_RecordRef" => {
            record_ref(&fields, "qualityDistributionRecord", notes)?
        }
        "CraftingQualityDistribution_Base" | "CraftingQualityDistribution_Base_NonRef" => {
            fields.check(&[], notes);
            ResourceQualityPayload::EmptyBase
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourceQualityPayload::Unsupported
        }
    };
    Ok(ResourceQualitySource {
        source_type: source.type_name(),
        payload,
    })
}

/// Projects the original source of a location-override declaration.
pub(crate) fn overrides<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualitySource<'source, ResourceQualityOverrides<'source>>, FieldError> {
    let fields = Fields::new(source, path);
    let payload = match source.type_name() {
        "CraftingQualityLocationOverride" => {
            fields.check(&["locationOverrideList"], notes);
            ResourceQualityPayload::Inline(InlineQuality {
                value: override_list(source, path, notes)?,
            })
        }
        "CraftingQualityLocationOverride_RecordRef" => {
            record_ref(&fields, "locationOverrideRecord", notes)?
        }
        "CraftingQualityLocationOverride_Base" | "CraftingQualityLocationOverride_Base_NonRef" => {
            fields.check(&[], notes);
            ResourceQualityPayload::EmptyBase
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourceQualityPayload::Unsupported
        }
    };
    Ok(ResourceQualitySource {
        source_type: source.type_name(),
        payload,
    })
}

fn override_list<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityOverrides<'source>, FieldError> {
    let fields = Fields::new(source, path);
    let location_override_list = fields
        .array("locationOverrideList")?
        .map(|values| {
            let mut entries = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let entry_path = format!("{path}.locationOverrideList[{index}]");
                let entry = source_fields::entry(value, source.type_name(), &entry_path)?;
                entries.push(
                    entry
                        .map(|entry| override_entry(entry, &entry_path, notes))
                        .transpose()?,
                );
            }
            Ok::<_, FieldError>(entries)
        })
        .transpose()?;
    Ok(ResourceQualityOverrides {
        location_override_list,
    })
}

fn override_entry<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityOverride<'source>, FieldError> {
    let fields = Fields::new(source, path);
    let payload = match source.type_name() {
        "CraftingQualityLocationOverrideEntry" => {
            fields.check(&["location", "qualityDistribution"], notes);
            let quality_distribution = fields
                .structure("qualityDistribution")?
                .map(|value| distribution(value, &format!("{path}.qualityDistribution"), notes))
                .transpose()?;
            ResourceQualityOverridePayload::Entry(ResourceQualityOverrideFields {
                location: fields.reference("location")?,
                quality_distribution,
            })
        }
        "CraftingQualityLocationOverrideEntry_Base"
        | "CraftingQualityLocationOverrideEntry_Base_NonRef" => {
            fields.check(&[], notes);
            ResourceQualityOverridePayload::EmptyBase
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourceQualityOverridePayload::Unsupported
        }
    };
    Ok(ResourceQualityOverride {
        source_type: source.type_name(),
        payload,
    })
}

/// Projects the authored inline bands or quantization Record link.
pub(crate) fn quantization<'source>(
    source: &StructValue<'source>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualitySource<'source, ResourceQualityQuantization>, FieldError> {
    let fields = Fields::new(source, path);
    let payload = match source.type_name() {
        "CraftingQualityQuantization" => {
            fields.check(&["bands"], notes);
            ResourceQualityPayload::Inline(InlineQuality {
                value: bands(source, path, notes)?,
            })
        }
        "CraftingQualityQuantization_RecordRef" => {
            record_ref(&fields, "qualityQuantizationRecord", notes)?
        }
        "CraftingQualityQuantization_Base" | "CraftingQualityQuantization_Base_NonRef" => {
            fields.check(&[], notes);
            ResourceQualityPayload::EmptyBase
        }
        _ => {
            notes.push(fields.unsupported(""));
            ResourceQualityPayload::Unsupported
        }
    };
    Ok(ResourceQualitySource {
        source_type: source.type_name(),
        payload,
    })
}

fn bands(
    source: &StructValue<'_>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityQuantization, FieldError> {
    let fields = Fields::new(source, path);
    let bands = fields
        .array("bands")?
        .map(|values| {
            let mut bands = Vec::with_capacity(values.len());
            for (index, value) in values.iter().enumerate() {
                let entry_path = format!("{path}.bands[{index}]");
                let entry = source_fields::entry(value, source.type_name(), &entry_path)?;
                let value = if let Some(entry) = entry {
                    band(entry, &entry_path, notes)?
                } else {
                    notes.push(FieldError::Malformed {
                        source_type: source.type_name().to_owned(),
                        field: entry_path,
                        expected: "a non-null CraftingQualityQuantizationBand Class value",
                    });
                    None
                };
                bands.push(value);
            }
            Ok::<_, FieldError>(bands)
        })
        .transpose()?;
    Ok(ResourceQualityQuantization { bands })
}

fn band(
    source: &StructValue<'_>,
    path: &str,
    notes: &mut Vec<FieldError>,
) -> Result<Option<QuantizationBand>, FieldError> {
    let fields = Fields::new(source, path);
    if source.type_name() != "CraftingQualityQuantizationBand" {
        notes.push(fields.unsupported(""));
        return Ok(None);
    }
    fields.check(&["start", "end", "mappedValue"], notes);
    let required = |name| {
        fields
            .int32(name)?
            .ok_or_else(|| fields.invalid(name, &Value::Null, "a non-null Int32"))
    };
    Ok(Some(QuantizationBand {
        start: i64::from(required("start")?),
        end: i64::from(required("end")?),
        mapped_value: i64::from(required("mappedValue")?),
    }))
}

fn record_ref<T>(
    fields: &Fields<'_, '_>,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityPayload<T>, FieldError> {
    fields.check(&[field], notes);
    Ok(ResourceQualityPayload::RecordRef(QualityRecordReference {
        reference: fields.reference(field)?,
    }))
}
