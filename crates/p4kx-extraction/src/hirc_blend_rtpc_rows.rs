// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{BankRtpcCurve, BankRtpcPoint, BankRtpcRow, BankRtpcRows};
use serde::ser::{Error as _, SerializeSeq};

use crate::source_evidence_error::SourceEvidenceError;

use super::super::dto::BlendSourceRow;
use super::{ObjectOrigins, Occurrence};

pub(super) fn serialize<R: Read + Seek, S: SerializeSeq>(
    sequence: &mut S,
    origins: &ObjectOrigins,
    layer: Occurrence<'_>,
    rows: BankRtpcRows<'_, R>,
) -> Result<(), S::Error> {
    let mut current_curve = None;
    for row in rows {
        let projected = match row.map_err(S::Error::custom)? {
            BankRtpcRow::Curve { ordinal, fields } => {
                current_curve = Some((ordinal, fields.rtpc_id.range.clone()));
                curve(origins, layer, ordinal, &fields)
            }
            BankRtpcRow::Point {
                curve,
                ordinal,
                fields,
            } => {
                let Some((_, curve_range)) =
                    current_curve.as_ref().filter(|(index, _)| *index == curve)
                else {
                    return Err(S::Error::custom(
                        "an initial RTPC point lacks its original curve",
                    ));
                };
                point(origins, layer, curve_range, curve, ordinal, &fields)
            }
            _ => return Err(S::Error::custom("unexpected initial RTPC row")),
        }
        .map_err(S::Error::custom)?;
        sequence.serialize_element(&projected)?;
    }
    Ok(())
}

fn curve(
    origins: &ObjectOrigins,
    layer: Occurrence<'_>,
    ordinal: u16,
    original: &BankRtpcCurve,
) -> Result<BlendSourceRow, SourceEvidenceError> {
    Ok(BlendSourceRow::RtpcCurve {
        layer_ordinal: layer.source(origins, "blendLayerPosition")?,
        curve_ordinal: origins.derived(
            ordinal,
            &original.rtpc_id.range,
            "blendRtpcCurvePosition",
        )?,
        rtpc_id: origins.field(&original.rtpc_id, "rtpcId")?,
        rtpc_type: origins.field(&original.rtpc_type, "rtpcType")?,
        accumulation: origins.field(&original.accumulation, "rtpcAccumulation")?,
        parameter_id: origins.field(&original.parameter_id, "rtpcParameterId")?,
        curve_id: origins.field(&original.curve_id, "rtpcCurveId")?,
        scaling: origins.field(&original.scaling, "rtpcScaling")?,
        point_count: origins.field(&original.point_count, "rtpcPointCount")?,
    })
}

fn point(
    origins: &ObjectOrigins,
    layer: Occurrence<'_>,
    curve_range: &Range<u64>,
    curve: u16,
    ordinal: u16,
    original: &BankRtpcPoint,
) -> Result<BlendSourceRow, SourceEvidenceError> {
    Ok(BlendSourceRow::RtpcPoint {
        layer_ordinal: layer.source(origins, "blendLayerPosition")?,
        curve_ordinal: origins.derived(curve, curve_range, "blendRtpcCurvePosition")?,
        point_ordinal: origins.derived(
            ordinal,
            &original.from_bits.range,
            "blendRtpcPointPosition",
        )?,
        from_bits: origins.field(&original.from_bits, "rtpcFromBits")?,
        to_bits: origins.field(&original.to_bits, "rtpcToBits")?,
        interpolation: origins.field(&original.interpolation, "rtpcInterpolation")?,
    })
}
