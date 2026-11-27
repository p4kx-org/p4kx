// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::bank::{
    BankInlineStateRow, BankInlineStateRows, BankRtpcRow, BankRtpcRows, StateProperties,
};

use super::{Coverage, Input, Output, Owner, TestResult, Visitor};

pub(super) fn state(
    visitor: &mut Visitor<'_>,
    owner: Owner,
    rows: Option<BankInlineStateRows<'_, Input<'_>>>,
) -> TestResult {
    let Some(rows) = rows else {
        return Ok(());
    };
    if let Some(output) = &visitor.output {
        let row = output.row("audio-bank-hirc-inline-state", &owner)?;
        output.group_extent(row, "/payload/checked_range", rows.range())?;
    }
    for original in rows {
        let original = original?;
        if matches!(
            &original,
            BankInlineStateRow::State { .. } | BankInlineStateRow::Binding { .. }
        ) {
            visitor.mark(Coverage::InlineState);
        }
        if let Some(output) = &visitor.output {
            state_row(output, owner, original)?;
        }
    }
    Ok(())
}

fn state_row(
    output: &Output<'_>,
    [section, object]: Owner,
    original: BankInlineStateRow,
) -> TestResult {
    match original {
        BankInlineStateRow::BindingCount(count) => {
            let row = output.row(
                "audio-bank-hirc-inline-state-binding-count",
                &[section, object],
            )?;
            output.field(row, "/payload/count", Some(&count))
        }
        BankInlineStateRow::Binding { ordinal, fields } => {
            let row = output.row(
                "audio-bank-hirc-inline-state-binding",
                &[section, object, u64::from(ordinal)],
            )?;
            output.field(row, "/payload/property_id", Some(&fields.property_id))?;
            output.field(row, "/payload/accumulation", Some(&fields.accumulation))?;
            output.field(row, "/payload/in_db", Some(&fields.in_db))
        }
        BankInlineStateRow::GroupCount(count) => {
            let row = output.row(
                "audio-bank-hirc-inline-state-group-count",
                &[section, object],
            )?;
            output.field(row, "/payload/count", Some(&count))
        }
        BankInlineStateRow::Group { ordinal, fields } => {
            let row = output.row(
                "audio-bank-hirc-inline-state-group",
                &[section, object, u64::from(ordinal)],
            )?;
            output.field(row, "/payload/group_id", Some(&fields.group_id))?;
            output.field(
                row,
                "/payload/synchronization",
                Some(&fields.synchronization),
            )?;
            output.field(row, "/payload/state_count", Some(&fields.state_count))
        }
        BankInlineStateRow::State {
            group,
            ordinal,
            state_id,
            properties,
            ..
        } => {
            let indices = [section, object, u64::from(group), u64::from(ordinal)];
            let row = output.row("audio-bank-hirc-inline-state-entry", &indices)?;
            output.field(row, "/payload/state_id", Some(&state_id))?;
            output.field(row, "/payload/property_count", Some(properties.count()))?;
            state_values(
                output,
                "audio-bank-hirc-inline-state-property",
                &indices,
                &properties,
            )
        }
        _ => Err("unexpected original State row".into()),
    }
}

pub(super) fn state_values(
    output: &Output<'_>,
    namespace: &str,
    owner: &[u64],
    source: &StateProperties,
) -> TestResult {
    for (ordinal, original) in source.entries().enumerate() {
        let mut indices = owner.to_vec();
        indices.push(u64::try_from(ordinal)?);
        let row = output.row(namespace, &indices)?;
        output.field(row, "/payload/property_id", Some(&original.id))?;
        output.field(row, "/payload/value_bits", Some(&original.value_bits))?;
    }
    Ok(())
}

pub(super) fn rtpc(
    visitor: &mut Visitor<'_>,
    owner: Owner,
    family: &str,
    rows: Option<BankRtpcRows<'_, Input<'_>>>,
) -> TestResult {
    let Some(rows) = rows else {
        return Ok(());
    };
    if rows.checked_curve_count() > 0 {
        visitor.mark(Coverage::Rtpc);
    }
    let Some(output) = &visitor.output else {
        return Ok(());
    };
    let prefix = format!("audio-bank-hirc-{family}-rtpc");
    let row = output.row(&prefix, &owner)?;
    output.field(
        row,
        "/payload/curve_count",
        Some(rows.declared_curve_count()),
    )?;
    output.group_extent(row, "/payload/checked_range", rows.range())?;
    let curve = format!("{prefix}-curve");
    let point = format!("{prefix}-point");
    for original in rows {
        rtpc_row(output, owner, &curve, &point, original?)?;
    }
    Ok(())
}

fn rtpc_row(
    output: &Output<'_>,
    [section, object]: Owner,
    curve: &str,
    point: &str,
    original: BankRtpcRow,
) -> TestResult {
    match original {
        BankRtpcRow::Curve { ordinal, fields } => {
            let row = output.row(curve, &[section, object, u64::from(ordinal)])?;
            output.field(row, "/payload/rtpc_id", Some(&fields.rtpc_id))?;
            output.field(row, "/payload/rtpc_type", Some(&fields.rtpc_type))?;
            output.field(row, "/payload/accumulation", Some(&fields.accumulation))?;
            output.field(row, "/payload/parameter_id", Some(&fields.parameter_id))?;
            output.field(row, "/payload/curve_id", Some(&fields.curve_id))?;
            output.field(row, "/payload/scaling", Some(&fields.scaling))?;
            output.field(row, "/payload/point_count", Some(&fields.point_count))
        }
        BankRtpcRow::Point {
            curve,
            ordinal,
            fields,
        } => {
            let row = output.row(
                point,
                &[section, object, u64::from(curve), u64::from(ordinal)],
            )?;
            output.field(row, "/payload/from_bits", Some(&fields.from_bits))?;
            output.field(row, "/payload/to_bits", Some(&fields.to_bits))?;
            output.field(row, "/payload/interpolation", Some(&fields.interpolation))
        }
        _ => Err("unexpected original RTPC row".into()),
    }
}
