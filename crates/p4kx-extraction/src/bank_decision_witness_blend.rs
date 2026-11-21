// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_audio::{BankBlendAssociations, BankHircContainerNode, BankRtpcRow, BankRtpcRows};
use serde_json::json;

use super::super::values::{Projection, Rows};
use super::super::{Input, TestResult};

pub(in super::super) fn blend(
    node: &mut BankHircContainerNode<'_, Input<'_>>,
    projection: &Projection<'_>,
) -> TestResult<bool> {
    let header = projection.document.get("fields").ok_or("Blend header")?;
    projection.optional(header, "/child_count", node.fields().child_count.as_ref())?;
    if let Some(fields) = &node.fields().blend {
        projection.optional(
            header,
            "/continuous_validation",
            fields.continuous_validation.as_ref(),
        )?;
        if let Some(layers) = &fields.layers {
            projection.field(header, "/layers/declared_count", &layers.declared_count)?;
        }
    }
    let mut rows = projection.rows("/rows")?;
    if let Some(children) = node.children() {
        for child in children {
            let child = child?;
            let row = rows.kind("child")?;
            projection.at(row, "/ordinal", &json!(child.ordinal))?;
            projection.field(row, "/child_id", &child.child_id)?;
        }
    }
    let mut observed = false;
    if let Some(mut layers) = node.blend_layers() {
        while let Some(mut layer) = layers.next_layer()? {
            let row = rows.kind("layer")?;
            let ordinal = layer.ordinal;
            let fields = &layer.fields;
            projection.at(row, "/layer_ordinal", &json!(ordinal))?;
            projection.field(row, "/layer_id", &fields.layer_id)?;
            projection.optional(row, "/crossfade_rtpc_id", fields.crossfade_rtpc_id.as_ref())?;
            projection.optional(
                row,
                "/crossfade_rtpc_type",
                fields.crossfade_rtpc_type.as_ref(),
            )?;
            if let Some(extent) = &fields.rtpc {
                projection.field(row, "/initial_rtpc/declared_count", &extent.curve_count)?;
            }
            if let Some(extent) = &fields.associations {
                projection.field(row, "/associations/declared_count", &extent.declared_count)?;
            }
            if let Some(curves) = layer.rtpc() {
                initial_rtpc(projection, &mut rows, ordinal, curves)?;
            }
            if let Some(source) = layer.associations() {
                observed |= crossfade(projection, &mut rows, ordinal, source)?;
            }
        }
    }
    rows.finish()?;
    Ok(observed)
}

fn initial_rtpc(
    projection: &Projection<'_>,
    rows: &mut Rows<'_>,
    layer: u32,
    source: BankRtpcRows<'_, Input<'_>>,
) -> TestResult {
    for original in source {
        match original? {
            BankRtpcRow::Curve { ordinal, fields } => {
                let row = rows.kind("rtpc-curve")?;
                projection.at(row, "/layer_ordinal", &json!(layer))?;
                projection.at(row, "/curve_ordinal", &json!(ordinal))?;
                projection.field(row, "/rtpc_id", &fields.rtpc_id)?;
                projection.field(row, "/rtpc_type", &fields.rtpc_type)?;
                projection.field(row, "/accumulation", &fields.accumulation)?;
                projection.field(row, "/parameter_id", &fields.parameter_id)?;
                projection.field(row, "/curve_id", &fields.curve_id)?;
                projection.field(row, "/scaling", &fields.scaling)?;
                projection.field(row, "/point_count", &fields.point_count)?;
            }
            BankRtpcRow::Point {
                curve,
                ordinal,
                fields,
            } => {
                let row = rows.kind("rtpc-point")?;
                projection.at(row, "/layer_ordinal", &json!(layer))?;
                projection.at(row, "/curve_ordinal", &json!(curve))?;
                projection.at(row, "/point_ordinal", &json!(ordinal))?;
                projection.field(row, "/from_bits", &fields.from_bits)?;
                projection.field(row, "/to_bits", &fields.to_bits)?;
                projection.field(row, "/interpolation", &fields.interpolation)?;
            }
            _ => return Err("known original Blend RTPC row".into()),
        }
    }
    Ok(())
}

fn crossfade(
    projection: &Projection<'_>,
    rows: &mut Rows<'_>,
    layer: u32,
    mut source: BankBlendAssociations<'_, Input<'_>>,
) -> TestResult<bool> {
    let mut observed = false;
    while let Some(mut association) = source.next_association()? {
        let row = rows.kind("association")?;
        let ordinal = association.ordinal;
        projection.at(row, "/layer_ordinal", &json!(layer))?;
        projection.at(row, "/association_ordinal", &json!(ordinal))?;
        projection.field(row, "/child_id", &association.fields.child_id)?;
        if let Some(extent) = &association.fields.points {
            projection.field(row, "/points/declared_count", &extent.declared_count)?;
        }
        if let Some(points) = association.points() {
            for point in points {
                let point = point?;
                let row = rows.kind("crossfade-point")?;
                projection.at(row, "/layer_ordinal", &json!(layer))?;
                projection.at(row, "/association_ordinal", &json!(ordinal))?;
                projection.at(row, "/point_ordinal", &json!(point.ordinal))?;
                projection.field(row, "/from_bits", &point.from_bits)?;
                projection.optional(row, "/to_bits", point.to_bits.as_ref())?;
                projection.optional(row, "/interpolation", point.interpolation.as_ref())?;
                observed = true;
            }
        }
    }
    Ok(observed)
}
