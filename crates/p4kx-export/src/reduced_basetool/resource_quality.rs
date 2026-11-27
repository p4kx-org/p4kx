// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_link::RecordLink;
use crate::quality_source::{
    QualityCurveSource, QualityOverridePayload, QualityOverridesSource, QualityPayload,
    QualityQuantizationSource, QualitySource,
};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Quality<'source, T> {
    source_type: Cell<&'source str>,
    payload: Cell<Payload<'source, T>>,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Payload<'source, T> {
    Inline {
        value: Cell<T>,
    },
    RecordRef {
        reference: Cell<RecordLink<'source>>,
    },
    EmptyBase,
}

pub(super) fn distribution<'source>(
    path: &'static str,
    source: &'source SourcedValue<QualitySource<'_, QualityCurveSource>>,
) -> Result<Cell<Quality<'source, Curve<'source>>>, ReducedError> {
    quality(path, source, |curve| {
        Ok(Curve {
            min: completeness::cell(path, curve.min())?.map(Number::from),
            max: completeness::cell(path, curve.max())?.map(Number::from),
            mean: completeness::cell(path, curve.mean())?.map(Number::from),
            stddev: completeness::cell(path, curve.stddev())?.map(Number::from),
        })
    })
}

pub(super) fn location_overrides<'source>(
    path: &'static str,
    source: &'source SourcedValue<QualitySource<'_, QualityOverridesSource<'_>>>,
) -> Result<Cell<Quality<'source, Overrides<'source>>>, ReducedError> {
    quality(path, source, |overrides| {
        let location_override_list =
            completeness::list(path, overrides.location_override_list(), |entry| {
                let payload = completeness::cell(path, entry.payload())?.try_map(|payload| {
                    Ok::<_, ReducedError>(match payload {
                        QualityOverridePayload::Entry(fields) => OverridePayload::Entry {
                            location: completeness::cell(path, fields.location())?
                                .map(RecordLink::from),
                            quality_distribution: Box::new(distribution(
                                path,
                                fields.quality_distribution(),
                            )?),
                        },
                        QualityOverridePayload::EmptyBase => OverridePayload::EmptyBase,
                    })
                })?;
                Ok(Override {
                    source_type: completeness::cell(path, entry.source_type())?.map(|name| *name),
                    payload,
                })
            })?;
        Ok(Overrides {
            location_override_list,
        })
    })
}

pub(super) fn quantization<'source>(
    path: &'static str,
    source: &'source SourcedValue<QualitySource<'_, QualityQuantizationSource<'_>>>,
) -> Result<Cell<Quality<'source, Quantization<'source>>>, ReducedError> {
    quality(path, source, |quantization| {
        Ok(Quantization {
            bands: completeness::list(path, quantization.bands(), |band| {
                Ok(Band {
                    source_type: completeness::cell(path, band.source_type())?.map(|name| *name),
                    start: completeness::cell(path, band.start())?.map(Number::from),
                    end: completeness::cell(path, band.end())?.map(Number::from),
                    mapped_value: completeness::cell(path, band.mapped_value())?.map(Number::from),
                })
            })?,
        })
    })
}

fn quality<'source, S, T>(
    path: &'static str,
    value: &'source SourcedValue<QualitySource<'_, S>>,
    read_inline: impl FnOnce(&'source S) -> Result<T, ReducedError>,
) -> Result<Cell<Quality<'source, T>>, ReducedError> {
    completeness::cell(path, value)?.try_map(|source| {
        let payload = completeness::cell(path, source.payload())?.try_map(|payload| {
            Ok::<_, ReducedError>(match payload {
                QualityPayload::Inline(inline) => Payload::Inline {
                    value: completeness::cell(path, inline.value())?.try_map(read_inline)?,
                },
                QualityPayload::RecordRef(reference) => Payload::RecordRef {
                    reference: completeness::cell(path, reference.reference())?
                        .map(RecordLink::from),
                },
                QualityPayload::EmptyBase => Payload::EmptyBase,
            })
        })?;
        Ok(Quality {
            source_type: completeness::cell(path, source.source_type())?.map(|name| *name),
            payload,
        })
    })
}

#[derive(Serialize)]
pub(super) struct Curve<'source> {
    min: Cell<Number<'source>>,
    max: Cell<Number<'source>>,
    mean: Cell<Number<'source>>,
    stddev: Cell<Number<'source>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Overrides<'source> {
    location_override_list: Cell<Vec<Cell<Override<'source>>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Override<'source> {
    source_type: Cell<&'source str>,
    payload: Cell<OverridePayload<'source>>,
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum OverridePayload<'source> {
    Entry {
        location: Cell<RecordLink<'source>>,
        quality_distribution: Box<Cell<Quality<'source, Curve<'source>>>>,
    },
    EmptyBase,
}

#[derive(Serialize)]
pub(super) struct Quantization<'source> {
    bands: Cell<Vec<Cell<Band<'source>>>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Band<'source> {
    source_type: Cell<&'source str>,
    start: Cell<Number<'source>>,
    end: Cell<Number<'source>>,
    mapped_value: Cell<Number<'source>>,
}
