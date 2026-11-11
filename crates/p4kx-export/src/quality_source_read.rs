// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValue, StructValue};

use crate::CatalogError;
use crate::catalog_read::{CatalogFields, CatalogRecordContext};
use crate::quality_bands_read;
use crate::quality_curve_source_read;
use crate::quality_source::{
    InlineQuality, QualityCurveSource, QualityOverrideFields, QualityOverridePayload,
    QualityOverrideSource, QualityOverridesSource, QualityPayload, QualityQuantizationSource,
    QualityRecordReference, QualitySource,
};
use crate::resource_value_read;
use crate::source_fields::{FieldCapture, FieldError};

/// Projects an authored distribution without following its Record Reference.
pub(crate) fn distribution<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualitySource<'f, QualityCurveSource>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let payload = match source.type_name() {
        "CraftingQualityDistributionNormal" | "CraftingQualityDistributionUniform" => {
            Some(QualityPayload::Inline(InlineQuality {
                value: context.value(
                    path,
                    SourceValue::Present(quality_curve_source_read::read(
                        source, path, context, notes, capture,
                    )?),
                )?,
            }))
        }
        "CraftingQualityDistribution_RecordRef" => {
            Some(record_ref(&fields, "qualityDistributionRecord", notes)?)
        }
        "CraftingQualityDistribution_Base" | "CraftingQualityDistribution_Base_NonRef" => {
            fields.check_observed(&[], notes)?;
            Some(QualityPayload::EmptyBase)
        }
        _ => None,
    };
    wrap(source, path, context, notes, payload, capture)
}

/// Projects the ordered authored location-override source.
pub(crate) fn overrides<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualitySource<'f, QualityOverridesSource<'f>>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let payload = match source.type_name() {
        "CraftingQualityLocationOverride" => {
            fields.check_observed(&["locationOverrideList"], notes)?;
            Some(QualityPayload::Inline(InlineQuality {
                value: context.value(
                    path,
                    SourceValue::Present(override_list(source, path, context, notes, capture)?),
                )?,
            }))
        }
        "CraftingQualityLocationOverride_RecordRef" => {
            Some(record_ref(&fields, "locationOverrideRecord", notes)?)
        }
        "CraftingQualityLocationOverride_Base" | "CraftingQualityLocationOverride_Base_NonRef" => {
            fields.check_observed(&[], notes)?;
            Some(QualityPayload::EmptyBase)
        }
        _ => None,
    };
    wrap(source, path, context, notes, payload, capture)
}

fn override_list<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualityOverridesSource<'f>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let location_override_list =
        fields
            .array("locationOverrideList", notes)?
            .try_map(|values| {
                values
                    .iter()
                    .enumerate()
                    .map(|(index, value)| {
                        let location = format!("{path}.locationOverrideList[{index}]");
                        fields
                            .structure_element(
                                &format!("locationOverrideList[{index}]"),
                                value,
                                notes,
                            )?
                            .try_map(|entry| {
                                override_entry(entry, &location, context, notes, capture)
                            })
                    })
                    .collect::<Result<_, CatalogError>>()
            })?;
    Ok(QualityOverridesSource {
        location_override_list,
    })
}

fn override_entry<'f>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualityOverrideSource<'f>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let payload = match source.type_name() {
        "CraftingQualityLocationOverrideEntry" => {
            fields.check_observed(&["location", "qualityDistribution"], notes)?;
            let quality_distribution =
                fields
                    .structure("qualityDistribution", notes)?
                    .try_map(|value| {
                        distribution(
                            value,
                            &format!("{path}.qualityDistribution"),
                            context,
                            notes,
                            capture,
                        )
                    })?;
            Some(QualityOverridePayload::Entry(Box::new(
                QualityOverrideFields {
                    location: resource_value_read::reference(&fields, "location", notes)?,
                    quality_distribution,
                },
            )))
        }
        "CraftingQualityLocationOverrideEntry_Base"
        | "CraftingQualityLocationOverrideEntry_Base_NonRef" => {
            fields.check_observed(&[], notes)?;
            Some(QualityOverridePayload::EmptyBase)
        }
        _ => None,
    };
    Ok(QualityOverrideSource {
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
    capture: &dyn FieldCapture<'f>,
) -> Result<QualitySource<'f, QualityQuantizationSource<'f>>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    let payload = match source.type_name() {
        "CraftingQualityQuantization" => Some(QualityPayload::Inline(InlineQuality {
            value: context.value(
                path,
                SourceValue::Present(quality_bands_read::read(
                    source, path, context, notes, capture,
                )?),
            )?,
        })),
        "CraftingQualityQuantization_RecordRef" => {
            Some(record_ref(&fields, "qualityQuantizationRecord", notes)?)
        }
        "CraftingQualityQuantization_Base" | "CraftingQualityQuantization_Base_NonRef" => {
            fields.check_observed(&[], notes)?;
            Some(QualityPayload::EmptyBase)
        }
        _ => None,
    };
    wrap(source, path, context, notes, payload, capture)
}

fn record_ref<T>(
    fields: &CatalogFields<'_, '_, '_>,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<QualityPayload<T>, CatalogError> {
    fields.check_observed(&[field], notes)?;
    Ok(QualityPayload::RecordRef(QualityRecordReference {
        reference: resource_value_read::reference(fields, field, notes)?,
    }))
}

fn wrap<'f, T>(
    source: &StructValue<'f>,
    path: &str,
    context: &CatalogRecordContext<'_>,
    notes: &mut Vec<FieldError>,
    payload: Option<QualityPayload<T>>,
    capture: &dyn FieldCapture<'f>,
) -> Result<QualitySource<'f, T>, CatalogError> {
    let fields = CatalogFields::observed(source, path, context, capture)?;
    Ok(QualitySource {
        source_type: fields.source_type()?,
        payload: payload.map_or_else(
            || fields.unsupported(notes),
            |payload| context.selector(&format!("{path}.$type"), SourceValue::Present(payload)),
        )?,
    })
}
