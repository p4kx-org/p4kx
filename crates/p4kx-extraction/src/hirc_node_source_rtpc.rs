// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{BankRtpcRow, BankRtpcRows};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_value::SourcedValue;

use super::super::StateRange;
use super::ObjectOrigins;
use super::reader::NodeReader;

pub(super) struct RtpcGroup<'view, 'source, 'input, R> {
    pub(super) origins: &'view ObjectOrigins,
    pub(super) node: &'view RefCell<NodeReader<'source, 'input, R>>,
}

impl<R: Read + Seek> Serialize for RtpcGroup<'_, '_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut node = self.node.try_borrow_mut().map_err(S::Error::custom)?;
        let rows = node
            .rtpc()
            .ok_or_else(|| S::Error::custom("the checked RTPC region is absent"))?;
        Rtpc::read::<S::Error>(self.origins, rows)?.serialize(serializer)
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct Rtpc<Rows> {
    declared_curve_count: SourcedValue<u16>,
    checked_curve_count: SourcedValue<u16>,
    checked_range: SourcedValue<StateRange>,
    complete: SourcedValue<bool>,
    #[schemars(with = "Vec<RtpcRow>")]
    rows: Rows,
}

pub(super) type RtpcSchema = Rtpc<Vec<RtpcRow>>;

impl<'input, R> Rtpc<RtpcRows<'_, 'input, R>> {
    pub(super) fn read<'view, E: serde::ser::Error>(
        origins: &'view ObjectOrigins,
        rows: BankRtpcRows<'input, R>,
    ) -> Result<Rtpc<RtpcRows<'view, 'input, R>>, E> {
        let count = rows.declared_curve_count();
        let checked = rows.checked_curve_count();
        let range = rows.range();
        Ok(Rtpc {
            declared_curve_count: origins
                .field(count, "nodeRtpcCurveCount")
                .map_err(E::custom)?,
            checked_curve_count: origins
                .derived(checked, range, "nodeRtpcCheckedCurves")
                .map_err(E::custom)?,
            checked_range: origins
                .range(range, "nodeRtpcCheckedExtent")
                .map_err(E::custom)?,
            complete: origins
                .derived(checked == count.value, range, "nodeRtpcCoverage")
                .map_err(E::custom)?,
            rows: RtpcRows {
                origins,
                rows: RefCell::new(rows),
            },
        })
    }
}

pub(super) struct RtpcRows<'view, 'input, R> {
    origins: &'view ObjectOrigins,
    rows: RefCell<BankRtpcRows<'input, R>>,
}

impl<R: Read + Seek> Serialize for RtpcRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows = self.rows.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        for row in &mut *rows {
            sequence.serialize_element(&RtpcRow::read(
                self.origins,
                row.map_err(S::Error::custom)?,
            )?)?;
        }
        sequence.end()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum RtpcRow {
    Curve {
        ordinal: SourcedValue<u16>,
        rtpc_id: SourcedValue<u32>,
        rtpc_type: SourcedValue<u8>,
        accumulation: SourcedValue<u8>,
        parameter_id: SourcedValue<u32>,
        curve_id: SourcedValue<u32>,
        scaling: SourcedValue<u8>,
        point_count: SourcedValue<u16>,
    },
    Point {
        curve_ordinal: SourcedValue<u16>,
        ordinal: SourcedValue<u16>,
        from_bits: SourcedValue<u32>,
        to_bits: SourcedValue<u32>,
        interpolation: SourcedValue<u32>,
    },
}

impl RtpcRow {
    fn read<E: serde::ser::Error>(origins: &ObjectOrigins, row: BankRtpcRow) -> Result<Self, E> {
        Ok(match row {
            BankRtpcRow::Curve { ordinal, fields } => Self::Curve {
                ordinal: origins
                    .derived(ordinal, &fields.rtpc_id.range, "nodeRtpcCurvePosition")
                    .map_err(E::custom)?,
                rtpc_id: origins
                    .field(&fields.rtpc_id, "nodeRtpcId")
                    .map_err(E::custom)?,
                rtpc_type: origins
                    .field(&fields.rtpc_type, "nodeRtpcType")
                    .map_err(E::custom)?,
                accumulation: origins
                    .field(&fields.accumulation, "nodeRtpcAccumulation")
                    .map_err(E::custom)?,
                parameter_id: origins
                    .field(&fields.parameter_id, "nodeRtpcParameterId")
                    .map_err(E::custom)?,
                curve_id: origins
                    .field(&fields.curve_id, "nodeRtpcCurveId")
                    .map_err(E::custom)?,
                scaling: origins
                    .field(&fields.scaling, "nodeRtpcScaling")
                    .map_err(E::custom)?,
                point_count: origins
                    .field(&fields.point_count, "nodeRtpcPointCount")
                    .map_err(E::custom)?,
            },
            BankRtpcRow::Point {
                curve,
                ordinal,
                fields,
            } => Self::Point {
                curve_ordinal: origins
                    .derived(curve, &fields.from_bits.range, "nodeRtpcPointCurvePosition")
                    .map_err(E::custom)?,
                ordinal: origins
                    .derived(ordinal, &fields.from_bits.range, "nodeRtpcPointPosition")
                    .map_err(E::custom)?,
                from_bits: origins
                    .field(&fields.from_bits, "nodeRtpcFromBits")
                    .map_err(E::custom)?,
                to_bits: origins
                    .field(&fields.to_bits, "nodeRtpcToBits")
                    .map_err(E::custom)?,
                interpolation: origins
                    .field(&fields.interpolation, "nodeRtpcInterpolation")
                    .map_err(E::custom)?,
            },
            _ => return Err(E::custom("unexpected original RTPC row")),
        })
    }
}
