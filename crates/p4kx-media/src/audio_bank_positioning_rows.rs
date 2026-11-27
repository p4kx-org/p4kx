// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircField, BankPositioningRow, BankPositioningRowFields, BankPositioningRows,
    BankPositioningTableKind, BankPositioningVector,
};

use crate::audio_bank_node_declarations::Declarations;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_playback_payload::{
    NodePlaybackPayload, PositioningCoordinate, PositioningPathFields, PositioningPathRecord,
    PositioningRowLayout,
};
use crate::audio_bank_origins::BankOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-node-positioning-path";

pub(crate) fn emit<R: Read + Seek>(
    declarations: &Declarations<'_, '_>,
    kind: BankPositioningTableKind,
    rows: BankPositioningRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::Positioning);
    let table = match kind {
        BankPositioningTableKind::Vertex => 0,
        BankPositioningTableKind::Playlist => 1,
        BankPositioningTableKind::AutomationRange => 2,
        _ => return Err(MediaError::Contract("unexpected positioning table".into())),
    };
    for row in rows {
        let row = row.map_err(|cause| declarations.bank.replay_failure(cause))?;
        emit_row(declarations, &sites, table, &row, writer)?;
    }
    Ok(())
}

fn emit_row(
    declarations: &Declarations<'_, '_>,
    sites: &NodeSites,
    table: u64,
    row: &BankPositioningRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let range = &row.checked_range;
    let mut prepared = sites.record(declarations.bank.input, writer, Some(range))?;
    sites.converted(
        &mut prepared,
        "ordinal",
        range,
        "nodePositioningRowPosition",
    )?;
    sites.converted(&mut prepared, "layout", range, "nodePositioningRowCoverage")?;
    sites.converted(
        &mut prepared,
        "fields/type",
        range,
        "nodePositioningTableKind",
    )?;
    let fields = observations(sites, &mut prepared, &row.fields)?;
    let (layout, status, issue) = if row.complete {
        (PositioningRowLayout::Decoded, Status::Complete, None)
    } else {
        sites.issues(&mut prepared, range)?;
        (
            PositioningRowLayout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.truncated-node-positioning-row",
                "HIRC",
                Severity::Error,
                "the original positioning row ends before all fields fit",
            )),
        )
    };
    let record = PositioningPathRecord {
        layout,
        ordinal: row.ordinal,
        fields,
    };
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            NAMESPACE,
            vec![section, object, table, u64::from(row.ordinal)],
        ),
        status,
        &NodePlaybackPayload::Path(&record),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankPositioningRowFields,
) -> Result<PositioningPathFields, MediaError> {
    Ok(match fields {
        BankPositioningRowFields::Vertex {
            coordinates,
            duration,
            ..
        } => {
            return vertex(sites, prepared, coordinates, duration.as_ref());
        }
        BankPositioningRowFields::Playlist {
            vertices_offset,
            vertex_count,
            ..
        } => {
            sites.field(
                prepared,
                "fields/vertices_offset",
                &vertices_offset.range,
                "nodePathVerticesOffset",
            )?;
            PositioningPathFields::Playlist {
                vertices_offset: vertices_offset.into(),
                vertex_count: sites.scalar(
                    prepared,
                    "fields/vertex_count",
                    vertex_count.as_ref(),
                    "nodePathPlaylistVertexCount",
                )?,
            }
        }
        BankPositioningRowFields::AutomationRange(coordinates) => {
            let axes = vector(
                sites,
                prepared,
                coordinates,
                [
                    "nodePathRangeXBits",
                    "nodePathRangeYBits",
                    "nodePathRangeZBits",
                ],
            )?;
            PositioningPathFields::AutomationRange {
                x: axes.x,
                y: axes.y,
                z: axes.z,
            }
        }
        _ => return Err(MediaError::Contract("unexpected positioning row".into())),
    })
}

fn vertex(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    coordinates: &BankPositioningVector,
    duration: Option<&BankHircField<i32>>,
) -> Result<PositioningPathFields, MediaError> {
    let axes = vector(
        sites,
        prepared,
        coordinates,
        [
            "nodePathVertexXBits",
            "nodePathVertexYBits",
            "nodePathVertexZBits",
        ],
    )?;
    Ok(PositioningPathFields::Vertex {
        x: axes.x,
        y: axes.y,
        z: axes.z,
        duration: sites.scalar(
            prepared,
            "fields/duration",
            duration,
            "nodePathVertexDuration",
        )?,
    })
}

struct AxisValues {
    x: PositioningCoordinate,
    y: Option<PositioningCoordinate>,
    z: Option<PositioningCoordinate>,
}

fn vector(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    words: &BankPositioningVector,
    [x_label, y_label, z_label]: [&'static str; 3],
) -> Result<AxisValues, MediaError> {
    Ok(AxisValues {
        x: coordinate(sites, prepared, "x", &words.x_bits, x_label)?,
        y: words
            .y_bits
            .as_ref()
            .map(|bits| coordinate(sites, prepared, "y", bits, y_label))
            .transpose()?,
        z: words
            .z_bits
            .as_ref()
            .map(|bits| coordinate(sites, prepared, "z", bits, z_label))
            .transpose()?,
    })
}

fn coordinate(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    axis: &str,
    original: &BankHircField<u32>,
    label: &'static str,
) -> Result<PositioningCoordinate, MediaError> {
    sites.field(
        prepared,
        &format!("fields/{axis}/bits"),
        &original.range,
        label,
    )?;
    let value = f32::from_bits(original.value);
    if value.is_finite() {
        sites.converted(
            prepared,
            &format!("fields/{axis}/finite_value"),
            &original.range,
            "nodePositioningFiniteCoordinate",
        )?;
    }
    Ok(PositioningCoordinate {
        bits: original.into(),
        finite_value: value.is_finite().then_some(value),
    })
}
