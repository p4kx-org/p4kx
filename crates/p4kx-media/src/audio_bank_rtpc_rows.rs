// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankRtpcCurve, BankRtpcPoint, BankRtpcRow, BankRtpcRows};

use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc_payload::{RtpcCurveRecord, RtpcPayload, RtpcPointRecord};
use crate::origin_input::MediaOriginInput;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(crate) const CURVE_NAMESPACE: &str = "audio-bank-hirc-sound-rtpc-curve";
pub(crate) const POINT_NAMESPACE: &str = "audio-bank-hirc-sound-rtpc-point";

pub(super) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    rows: BankRtpcRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for row in rows {
        match row.map_err(|cause| projection.replay_failure(cause))? {
            BankRtpcRow::Curve { ordinal, fields } => {
                emit_curve(projection.input, sites, ordinal, &fields, writer)?;
            }
            BankRtpcRow::Point {
                curve,
                ordinal,
                fields,
            } => {
                emit_point(projection.input, sites, curve, ordinal, &fields, writer)?;
            }
            _ => return Err(MediaError::Contract("unexpected Sound RTPC row".into())),
        }
    }
    Ok(())
}

fn emit_curve(
    input: &MediaOriginInput,
    sites: &NodeSites,
    ordinal: u16,
    fields: &BankRtpcCurve,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(input, writer, Some(&fields.rtpc_id.range))?;
    for (name, range, original) in [
        ("rtpc_id", &fields.rtpc_id.range, "rtpcId"),
        ("rtpc_type", &fields.rtpc_type.range, "rtpcType"),
        (
            "accumulation",
            &fields.accumulation.range,
            "rtpcAccumulation",
        ),
        (
            "parameter_id",
            &fields.parameter_id.range,
            "rtpcParameterId",
        ),
        ("curve_id", &fields.curve_id.range, "rtpcCurveId"),
        ("scaling", &fields.scaling.range, "rtpcScaling"),
        ("point_count", &fields.point_count.range, "rtpcPointCount"),
    ] {
        sites.field(&mut prepared, name, range, original)?;
    }
    let record = RtpcCurveRecord {
        rtpc_id: (&fields.rtpc_id).into(),
        rtpc_type: (&fields.rtpc_type).into(),
        accumulation: (&fields.accumulation).into(),
        parameter_id: (&fields.parameter_id).into(),
        curve_id: (&fields.curve_id).into(),
        scaling: (&fields.scaling).into(),
        point_count: (&fields.point_count).into(),
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(CURVE_NAMESPACE, vec![section, object, u64::from(ordinal)]),
        Status::Complete,
        &RtpcPayload::Curve(&record),
        &[],
    )
}

fn emit_point(
    input: &MediaOriginInput,
    sites: &NodeSites,
    curve: u16,
    ordinal: u16,
    fields: &BankRtpcPoint,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut prepared = sites.record(input, writer, Some(&fields.from_bits.range))?;
    for (name, range, original, interpretation) in [
        (
            "from_bits",
            &fields.from_bits.range,
            "rtpcFromBits",
            "input",
        ),
        ("to_bits", &fields.to_bits.range, "rtpcToBits", "output"),
    ] {
        sites.field(&mut prepared, name, range, original)?;
        sites.converted(&mut prepared, interpretation, range, original)?;
    }
    sites.field(
        &mut prepared,
        "interpolation",
        &fields.interpolation.range,
        "rtpcInterpolation",
    )?;
    let input = fields.input();
    let output = fields.output();
    let record = RtpcPointRecord {
        from_bits: (&fields.from_bits).into(),
        to_bits: (&fields.to_bits).into(),
        interpolation: (&fields.interpolation).into(),
        input: input.is_finite().then_some(input),
        output: output.is_finite().then_some(output),
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            POINT_NAMESPACE,
            vec![section, object, u64::from(curve), u64::from(ordinal)],
        ),
        Status::Complete,
        &RtpcPayload::Point(&record),
        &[],
    )
}
