// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};
use std::ops::Range;

use p4kx_audio::{
    BankAttenuationCone, BankHircAttenuation, BankHircObject, BankObservedHeader, BankRtpcPoint,
};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_inventory::SourceInventory;
use crate::source_value::SourcedValue;

use super::super::node_source::rtpc::{Rtpc, RtpcSchema};
use super::container_layout::ContainerSourceLayout;
use super::origins::ObjectOrigins;

pub(in crate::hirc_view) type AttenuationSourceSchema =
    AttenuationDocument<Vec<AttenuationRow>, RtpcSchema, SourceInventory>;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
#[schemars(rename = "AttenuationObjectSourceFields")]
pub(in crate::hirc_view) struct AttenuationDocument<Rows, InitialRtpc, Inventory> {
    kind: AttenuationKind,
    layout: SourcedValue<ContainerSourceLayout>,
    version: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<SourcedValue<String>>,
    fields: AttenuationFields,
    #[schemars(with = "Vec<AttenuationRow>")]
    rows: Rows,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[schemars(with = "Option<RtpcSchema>")]
    initial_rtpc: Option<InitialRtpc>,
    #[schemars(with = "SourceInventory")]
    source_inventory: Inventory,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum AttenuationKind {
    Attenuation,
}

#[derive(Default, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct AttenuationFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    height_spread_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cone_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cone_enabled: Option<SourcedValue<bool>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cone: Option<ConeFields>,
    #[serde(skip_serializing_if = "Option::is_none")]
    curve_count: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    checked_range: Option<SourcedValue<StateRange>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    unprojected_range: Option<SourcedValue<StateRange>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct ConeFields {
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "inside_degrees_bits")]
    inside_degrees: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outside_degrees_bits")]
    outside_degrees: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "outside_volume_bits")]
    outside_volume: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "low_pass_bits")]
    low_pass: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "high_pass_bits")]
    high_pass: Option<SourcedValue<u32>>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub(in crate::hirc_view) enum AttenuationRow {
    Selector {
        ordinal: SourcedValue<u64>,
        curve_index: SourcedValue<i8>,
    },
    Curve {
        ordinal: SourcedValue<u64>,
        scaling: SourcedValue<u8>,
        #[serde(skip_serializing_if = "Option::is_none")]
        point_count: Option<SourcedValue<u16>>,
        points_range: SourcedValue<StateRange>,
    },
    Point {
        curve_ordinal: SourcedValue<u64>,
        ordinal: SourcedValue<u64>,
        from_bits: SourcedValue<u32>,
        to_bits: SourcedValue<u32>,
        interpolation: SourcedValue<u32>,
    },
}

pub(super) fn serialize<R: Read + Seek, S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    attenuation: BankHircAttenuation<'_, R>,
) -> Result<S::Ok, S::Error> {
    let original = attenuation.object();
    let format_error = attenuation.format_error();
    let layout = if format_error.is_some() {
        ContainerSourceLayout::Malformed
    } else {
        ContainerSourceLayout::Decoded
    };
    let error = format_error
        .map(|error| origins.coverage(error.to_string(), header, original))
        .transpose()
        .map_err(S::Error::custom)?;
    let layout = origins
        .coverage(layout, header, original)
        .map_err(S::Error::custom)?;
    let fields = fields(&attenuation, origins).map_err(S::Error::custom)?;
    let rtpc_available = attenuation.fields().rtpc.is_some();
    let reader = RefCell::new(attenuation);
    AttenuationDocument {
        kind: AttenuationKind::Attenuation,
        layout,
        version: origins.version(header).map_err(S::Error::custom)?,
        error,
        fields,
        rows: ConversionRows {
            reader: &reader,
            origins,
        },
        initial_rtpc: rtpc_available.then_some(InitialRtpcGroup {
            reader: &reader,
            origins,
        }),
        source_inventory: origins,
    }
    .serialize(serializer)
}

pub(super) fn unsupported<S: Serializer>(
    serializer: S,
    origins: &ObjectOrigins,
    header: &BankObservedHeader,
    object: &BankHircObject,
) -> Result<S::Ok, S::Error> {
    let extent = object.body_offset()..object.body_end_exclusive();
    AttenuationDocument {
        kind: AttenuationKind::Attenuation,
        layout: origins
            .coverage(ContainerSourceLayout::Unsupported, header, object)
            .map_err(S::Error::custom)?,
        version: origins.version(header).map_err(S::Error::custom)?,
        error: None,
        fields: AttenuationFields {
            unprojected_range: Some(
                origins
                    .range(&extent, "attenuationUnprojectedExtent")
                    .map_err(S::Error::custom)?,
            ),
            ..AttenuationFields::default()
        },
        rows: [] as [AttenuationRow; 0],
        initial_rtpc: None::<RtpcSchema>,
        source_inventory: origins,
    }
    .serialize(serializer)
}

fn fields<R>(
    attenuation: &BankHircAttenuation<'_, R>,
    origins: &ObjectOrigins,
) -> Result<AttenuationFields, SourceEvidenceError> {
    let fields = attenuation.fields();
    Ok(AttenuationFields {
        height_spread_flags: origins
            .optional(fields.height_spread_flags.as_ref(), "heightSpreadFlags")?,
        cone_flags: origins.optional(fields.cone_flags.as_ref(), "coneFlags")?,
        cone_enabled: fields
            .cone_flags
            .as_ref()
            .zip(fields.cone_enabled())
            .map(|(flags, value)| origins.derived(value, &flags.range, "coneEnabled"))
            .transpose()?,
        cone: fields
            .cone
            .as_ref()
            .map(|cone| cone_fields(origins, cone))
            .transpose()?,
        curve_count: origins.optional(fields.curve_count.as_ref(), "attenuationCurveCount")?,
        checked_range: Some(
            origins.range(&attenuation.checked_range(), "attenuationCheckedExtent")?,
        ),
        unprojected_range: attenuation
            .unprojected_range()
            .map(|range| origins.range(&range, "attenuationUnprojectedExtent"))
            .transpose()?,
    })
}

fn cone_fields(
    origins: &ObjectOrigins,
    cone: &BankAttenuationCone,
) -> Result<ConeFields, SourceEvidenceError> {
    Ok(ConeFields {
        inside_degrees: origins
            .optional(cone.inside_degrees_bits.as_ref(), "coneInsideDegreesBits")?,
        outside_degrees: origins
            .optional(cone.outside_degrees_bits.as_ref(), "coneOutsideDegreesBits")?,
        outside_volume: origins
            .optional(cone.outside_volume_bits.as_ref(), "coneOutsideVolumeBits")?,
        low_pass: origins.optional(cone.low_pass_bits.as_ref(), "coneLowPassBits")?,
        high_pass: origins.optional(cone.high_pass_bits.as_ref(), "coneHighPassBits")?,
    })
}

struct ConversionRows<'view, 'input, R> {
    reader: &'view RefCell<BankHircAttenuation<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<R: Read + Seek> Serialize for ConversionRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut reader = self.reader.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        for (ordinal, selector) in reader.fields().selectors.iter().enumerate() {
            sequence.serialize_element(&AttenuationRow::Selector {
                ordinal: ordinal_at::<S::Error>(self.origins, ordinal, &selector.range)?,
                curve_index: self
                    .origins
                    .field(selector, "attenuationSignedSelector")
                    .map_err(S::Error::custom)?,
            })?;
        }
        for ordinal in 0..reader.fields().curves.len() {
            let curve = reader
                .fields()
                .curves
                .get(ordinal)
                .ok_or_else(|| S::Error::custom("the selected attenuation curve is absent"))?;
            let scaling_range = curve.scaling.range.clone();
            sequence.serialize_element(&AttenuationRow::Curve {
                ordinal: ordinal_at::<S::Error>(self.origins, ordinal, &scaling_range)?,
                scaling: self
                    .origins
                    .field(&curve.scaling, "attenuationScaling")
                    .map_err(S::Error::custom)?,
                point_count: self
                    .origins
                    .optional(curve.point_count.as_ref(), "attenuationPointCount")
                    .map_err(S::Error::custom)?,
                points_range: self
                    .origins
                    .range(&curve.points_range, "attenuationPointExtent")
                    .map_err(S::Error::custom)?,
            })?;
            let points = reader.points(ordinal).ok_or_else(|| {
                S::Error::custom("the checked attenuation point extent is absent")
            })?;
            for (point_ordinal, point) in points.enumerate() {
                let point = point.map_err(S::Error::custom)?;
                sequence.serialize_element(&point_row::<S::Error>(
                    self.origins,
                    ordinal,
                    point_ordinal,
                    &scaling_range,
                    &point,
                )?)?;
            }
        }
        sequence.end()
    }
}

fn point_row<E: serde::ser::Error>(
    origins: &ObjectOrigins,
    curve_ordinal: usize,
    ordinal: usize,
    curve_range: &Range<u64>,
    point: &BankRtpcPoint,
) -> Result<AttenuationRow, E> {
    Ok(AttenuationRow::Point {
        curve_ordinal: ordinal_at::<E>(origins, curve_ordinal, curve_range)?,
        ordinal: ordinal_at::<E>(origins, ordinal, &point.from_bits.range)?,
        from_bits: origins
            .field(&point.from_bits, "attenuationFromBits")
            .map_err(E::custom)?,
        to_bits: origins
            .field(&point.to_bits, "attenuationToBits")
            .map_err(E::custom)?,
        interpolation: origins
            .field(&point.interpolation, "attenuationInterpolation")
            .map_err(E::custom)?,
    })
}

fn ordinal_at<E: serde::ser::Error>(
    origins: &ObjectOrigins,
    ordinal: usize,
    range: &Range<u64>,
) -> Result<SourcedValue<u64>, E> {
    origins
        .derived(
            u64::try_from(ordinal).map_err(E::custom)?,
            range,
            "attenuationRowPosition",
        )
        .map_err(E::custom)
}

struct InitialRtpcGroup<'view, 'input, R> {
    reader: &'view RefCell<BankHircAttenuation<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<R: Read + Seek> Serialize for InitialRtpcGroup<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut reader = self.reader.try_borrow_mut().map_err(S::Error::custom)?;
        let rows = reader
            .rtpc()
            .ok_or_else(|| S::Error::custom("the checked attenuation RTPC region is absent"))?;
        Rtpc::read::<S::Error>(self.origins, rows)?.serialize(serializer)
    }
}
