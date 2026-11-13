// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankNodePositioningFields, BankPositioningRow, BankPositioningRowFields,
    BankPositioningTableKind, BankPositioningVector,
};
use schemars::JsonSchema;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

use super::super::StateRange;
use super::ObjectOrigins;
use super::reader::NodeReader;

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct Positioning<Rows> {
    #[serde(skip_serializing_if = "Option::is_none")]
    flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    spatial_flags: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path_mode: Option<SourcedValue<u8>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transition_time: Option<SourcedValue<i32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    vertex_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    playlist_count: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    complete_range: Option<SourcedValue<StateRange>>,
    #[schemars(with = "Vec<PositioningRow>")]
    rows: Rows,
}

impl<Rows> Positioning<Rows> {
    pub(super) fn read(
        origins: &ObjectOrigins,
        fields: &BankNodePositioningFields,
        rows: Rows,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            flags: origins.optional(fields.flags.as_ref(), "nodePositioningFlags")?,
            spatial_flags: origins
                .optional(fields.spatial_flags.as_ref(), "nodePositioningSpatialFlags")?,
            path_mode: origins.optional(fields.path_mode.as_ref(), "nodePositioningPathMode")?,
            transition_time: origins.optional(
                fields.transition_time.as_ref(),
                "nodePositioningTransitionTime",
            )?,
            vertex_count: origins
                .optional(fields.vertex_count.as_ref(), "nodePositioningVertexCount")?,
            playlist_count: origins.optional(
                fields.playlist_count.as_ref(),
                "nodePositioningPlaylistCount",
            )?,
            complete_range: fields
                .complete_range()
                .map(|range| origins.range(&range, "nodePositioningExtent"))
                .transpose()?,
            rows,
        })
    }
}

pub(super) type PositioningSchema = Positioning<Vec<PositioningRow>>;

pub(super) struct PositioningRows<'view, 'source, 'input, R> {
    pub(super) origins: &'view ObjectOrigins,
    pub(super) node: &'view RefCell<NodeReader<'source, 'input, R>>,
}

impl<R: Read + Seek> Serialize for PositioningRows<'_, '_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut node = self.node.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        for kind in [
            BankPositioningTableKind::Vertex,
            BankPositioningTableKind::Playlist,
            BankPositioningTableKind::AutomationRange,
        ] {
            if let Some(rows) = node.positioning_rows(kind) {
                for row in rows {
                    let row = row.map_err(S::Error::custom)?;
                    sequence.serialize_element(&PositioningRow::read(self.origins, &row)?)?;
                }
            }
        }
        sequence.end()
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct PositioningRow {
    ordinal: SourcedValue<u32>,
    range: SourcedValue<StateRange>,
    complete: SourcedValue<bool>,
    fields: PositioningFields,
}

impl PositioningRow {
    fn read<E: serde::ser::Error>(
        origins: &ObjectOrigins,
        row: &BankPositioningRow,
    ) -> Result<Self, E> {
        let extent = &row.checked_range;
        Ok(Self {
            ordinal: origins
                .derived(row.ordinal, extent, "nodePositioningRowPosition")
                .map_err(E::custom)?,
            range: origins
                .range(extent, "nodePositioningRowExtent")
                .map_err(E::custom)?,
            complete: origins
                .derived(row.complete, extent, "nodePositioningRowCoverage")
                .map_err(E::custom)?,
            fields: PositioningFields::read(origins, &row.fields)?,
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum PositioningFields {
    Vertex {
        coordinates: PositioningVector,
        #[serde(skip_serializing_if = "Option::is_none")]
        duration: Option<SourcedValue<i32>>,
    },
    Playlist {
        vertices_offset: SourcedValue<u32>,
        #[serde(skip_serializing_if = "Option::is_none")]
        vertex_count: Option<SourcedValue<u32>>,
    },
    AutomationRange {
        coordinates: PositioningVector,
    },
}

impl PositioningFields {
    fn read<E: serde::ser::Error>(
        origins: &ObjectOrigins,
        fields: &BankPositioningRowFields,
    ) -> Result<Self, E> {
        Ok(match fields {
            BankPositioningRowFields::Vertex {
                coordinates,
                duration,
                ..
            } => Self::Vertex {
                coordinates: PositioningVector::read(origins, coordinates).map_err(E::custom)?,
                duration: origins
                    .optional(duration.as_ref(), "nodePathVertexDuration")
                    .map_err(E::custom)?,
            },
            BankPositioningRowFields::Playlist {
                vertices_offset,
                vertex_count,
                ..
            } => Self::Playlist {
                vertices_offset: origins
                    .field(vertices_offset, "nodePathVerticesOffset")
                    .map_err(E::custom)?,
                vertex_count: origins
                    .optional(vertex_count.as_ref(), "nodePathVertexCount")
                    .map_err(E::custom)?,
            },
            BankPositioningRowFields::AutomationRange(coordinates) => Self::AutomationRange {
                coordinates: PositioningVector::read(origins, coordinates).map_err(E::custom)?,
            },
            _ => return Err(E::custom("unexpected original positioning table")),
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
struct PositioningVector {
    #[serde(rename = "x_bits")]
    x: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "y_bits")]
    y: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    #[serde(rename = "z_bits")]
    z: Option<SourcedValue<u32>>,
}

impl PositioningVector {
    fn read(
        origins: &ObjectOrigins,
        vector: &BankPositioningVector,
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            x: origins.field(&vector.x_bits, "nodePositioningXBits")?,
            y: origins.optional(vector.y_bits.as_ref(), "nodePositioningYBits")?,
            z: origins.optional(vector.z_bits.as_ref(), "nodePositioningZBits")?,
        })
    }
}
