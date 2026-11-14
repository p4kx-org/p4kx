// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::bank::{BankNodePositioningFields, BankPositioningTableKind};

use crate::audio_bank_node_declarations::{Coverage, Declarations, Disposition};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_playback_payload::{
    NodePlaybackPayload, PositioningModes, PositioningObservations,
};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_payload::BankEntryRange;
use crate::{MediaError, MediaWriter, Subresource};

pub(crate) const NAMESPACE: &str = "audio-bank-hirc-node-positioning";
pub(crate) const TABLES: [BankPositioningTableKind; 3] = [
    BankPositioningTableKind::Vertex,
    BankPositioningTableKind::Playlist,
    BankPositioningTableKind::AutomationRange,
];

pub(crate) fn summary(
    declarations: &Declarations<'_, '_>,
    disposition: Disposition<'_>,
    original: Option<&BankNodePositioningFields>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = declarations.sites(NodeKind::Positioning);
    let mut prepared = sites.record(declarations.bank.input, writer, None)?;
    let observed = if let Some(fields) = original {
        observations(&sites, &mut prepared, fields)?
    } else {
        PositioningObservations::default()
    };
    let (payload, status, issue) = declarations.summary(
        &sites,
        &mut prepared,
        disposition,
        &Coverage::read(
            original.and_then(|fields| fields.start),
            original.and_then(BankNodePositioningFields::complete_range),
        ),
        observed,
        "audio.bank.invalid-node-positioning",
    )?;
    prepared.finish().write(
        writer,
        &Subresource::new(NAMESPACE, sites.indices().to_vec()),
        status,
        &NodePlaybackPayload::Positioning(&payload),
        issue.as_slice(),
    )
}

fn observations(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankNodePositioningFields,
) -> Result<PositioningObservations, MediaError> {
    let mut observed = fields_read(sites, prepared, fields)?;
    let tables = [
        (
            "observed/vertices_range",
            BankPositioningTableKind::Vertex,
            &mut observed.vertices_range,
        ),
        (
            "observed/playlist_range",
            BankPositioningTableKind::Playlist,
            &mut observed.playlist_range,
        ),
        (
            "observed/automation_range",
            BankPositioningTableKind::AutomationRange,
            &mut observed.automation_range,
        ),
    ];
    for (path, kind, target) in tables {
        *target = extent(sites, prepared, path, fields.table(kind))?;
    }
    Ok(observed)
}

fn fields_read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankNodePositioningFields,
) -> Result<PositioningObservations, MediaError> {
    Ok(PositioningObservations {
        modes: modes(sites, prepared, fields)?,
        flags: sites.scalar(
            prepared,
            "observed/flags",
            fields.flags.as_ref(),
            "nodePositioningFlags",
        )?,
        spatial_flags: sites.scalar(
            prepared,
            "observed/spatial_flags",
            fields.spatial_flags.as_ref(),
            "nodePositioningSpatialFlags",
        )?,
        path_mode: sites.scalar(
            prepared,
            "observed/path_mode",
            fields.path_mode.as_ref(),
            "nodePositioningPathMode",
        )?,
        transition_time: sites.scalar(
            prepared,
            "observed/transition_time",
            fields.transition_time.as_ref(),
            "nodePositioningTransitionTime",
        )?,
        vertex_count: sites.scalar(
            prepared,
            "observed/vertex_count",
            fields.vertex_count.as_ref(),
            "nodePositioningVertexCount",
        )?,
        playlist_count: sites.scalar(
            prepared,
            "observed/playlist_count",
            fields.playlist_count.as_ref(),
            "nodePositioningPlaylistCount",
        )?,
        ..PositioningObservations::default()
    })
}

fn modes(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    original: &BankNodePositioningFields,
) -> Result<Option<PositioningModes>, MediaError> {
    let (Some(flags), Some(modes)) = (&original.flags, original.modes()) else {
        return Ok(None);
    };
    for name in [
        "override_parent",
        "has_spatial_fields",
        "panner_type_code",
        "position_type_code",
    ] {
        sites.converted(
            prepared,
            &format!("observed/modes/{name}"),
            &flags.range,
            "nodePositioningModeBits",
        )?;
    }
    Ok(Some(PositioningModes {
        override_parent: modes.override_parent,
        has_spatial_fields: modes.has_spatial_fields,
        panner_type_code: modes.panner_type_code,
        position_type_code: modes.position_type_code,
    }))
}

fn extent(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    path: &str,
    original: Option<&Range<u64>>,
) -> Result<Option<BankEntryRange>, MediaError> {
    original
        .map(|range| {
            sites.range(prepared, path, range)?;
            Ok(range.into())
        })
        .transpose()
}
