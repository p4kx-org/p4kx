// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::resource_quality_curve;
use crate::resource_quality_source::{
    InlineQuality, QualityRecordReference, ResourceQualityCurve, ResourceQualityOverride,
    ResourceQualityOverrideFields, ResourceQualityOverridePayload, ResourceQualityOverrides,
    ResourceQualityPayload, ResourceQualityQuantization, ResourceQualitySource,
};
use crate::resource_quantization_read;
use crate::resource_value_read;
use crate::source_fields::FieldError;

/// Projects an authored distribution without following its Record Reference.
pub(crate) fn distribution<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualitySource<'f, ResourceQualityCurve<'f>>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let payload = match source.type_name() {
        "CraftingQualityDistributionNormal" | "CraftingQualityDistributionUniform" => {
            Some(ResourceQualityPayload::Inline(InlineQuality {
                value: context.value(
                    path,
                    SourceValue::Present(resource_quality_curve::read(
                        source, path, context, notes,
                    )?),
                )?,
            }))
        }
        "CraftingQualityDistribution_RecordRef" => {
            Some(record_ref(&fields, "qualityDistributionRecord", notes)?)
        }
        "CraftingQualityDistribution_Base" | "CraftingQualityDistribution_Base_NonRef" => {
            fields.check(&[], notes);
            Some(ResourceQualityPayload::EmptyBase)
        }
        _ => None,
    };
    wrap(source, path, context, notes, payload)
}

/// Projects the ordered authored location-override source.
pub(crate) fn overrides<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualitySource<'f, ResourceQualityOverrides<'f>>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let payload = match source.type_name() {
        "CraftingQualityLocationOverride" => {
            fields.check(&["locationOverrideList"], notes);
            Some(ResourceQualityPayload::Inline(InlineQuality {
                value: context.value(
                    path,
                    SourceValue::Present(override_list(source, path, context, notes)?),
                )?,
            }))
        }
        "CraftingQualityLocationOverride_RecordRef" => {
            Some(record_ref(&fields, "locationOverrideRecord", notes)?)
        }
        "CraftingQualityLocationOverride_Base" | "CraftingQualityLocationOverride_Base_NonRef" => {
            fields.check(&[], notes);
            Some(ResourceQualityPayload::EmptyBase)
        }
        _ => None,
    };
    wrap(source, path, context, notes, payload)
}

fn override_list<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityOverrides<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let location_override_list =
        fields
            .array("locationOverrideList", notes)?
            .try_map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let location = format!("{path}.locationOverrideList[{index}]");
                        context
                            .structure_element(value, source.type_name(), &location, notes)?
                            .try_map(|entry| override_entry(entry, &location, context, notes))
                    })
                    .collect::<Result<_, CatalogError>>()
            })?;
    Ok(ResourceQualityOverrides {
        location_override_list,
    })
}

fn override_entry<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityOverride<'f>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let payload = match source.type_name() {
        "CraftingQualityLocationOverrideEntry" => {
            fields.check(&["location", "qualityDistribution"], notes);
            let quality_distribution =
                fields
                    .structure("qualityDistribution", notes)?
                    .try_map(|value| {
                        distribution(
                            value,
                            &format!("{path}.qualityDistribution"),
                            context,
                            notes,
                        )
                    })?;
            Some(ResourceQualityOverridePayload::Entry(Box::new(
                ResourceQualityOverrideFields {
                    location: resource_value_read::reference(&fields, "location", notes)?,
                    quality_distribution,
                },
            )))
        }
        "CraftingQualityLocationOverrideEntry_Base"
        | "CraftingQualityLocationOverrideEntry_Base_NonRef" => {
            fields.check(&[], notes);
            Some(ResourceQualityOverridePayload::EmptyBase)
        }
        _ => None,
    };
    Ok(ResourceQualityOverride {
        source_type: fields.source_type()?,
        payload: payload.map_or_else(
            || fields.unsupported(notes),
            |payload| context.selector(&format!("{path}.$type"), SourceValue::Present(payload)),
        )?,
    })
}

/// Projects inline quantization bands or the original Record Reference.
pub(crate) fn quantization<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualitySource<'f, ResourceQualityQuantization<'f>>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    let payload = match source.type_name() {
        "CraftingQualityQuantization" => Some(ResourceQualityPayload::Inline(InlineQuality {
            value: context.value(
                path,
                SourceValue::Present(resource_quantization_read::read(
                    source, path, context, notes,
                )?),
            )?,
        })),
        "CraftingQualityQuantization_RecordRef" => {
            Some(record_ref(&fields, "qualityQuantizationRecord", notes)?)
        }
        "CraftingQualityQuantization_Base" | "CraftingQualityQuantization_Base_NonRef" => {
            fields.check(&[], notes);
            Some(ResourceQualityPayload::EmptyBase)
        }
        _ => None,
    };
    wrap(source, path, context, notes, payload)
}

fn record_ref<T>(
    fields: &CatalogFields<'_, '_, '_>,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<ResourceQualityPayload<T>, CatalogError> {
    fields.check(&[field], notes);
    Ok(ResourceQualityPayload::RecordRef(QualityRecordReference {
        reference: resource_value_read::reference(fields, field, notes)?,
    }))
}

fn wrap<'f, T>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    payload: Option<ResourceQualityPayload<T>>,
) -> Result<ResourceQualitySource<'f, T>, CatalogError> {
    let fields = CatalogFields::new(source, path, context);
    Ok(ResourceQualitySource {
        source_type: fields.source_type()?,
        payload: payload.map_or_else(
            || fields.unsupported(notes),
            |payload| context.selector(&format!("{path}.$type"), SourceValue::Present(payload)),
        )?,
    })
}
