// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankBlendLayer, BankBlendLayerFields, BankHircObject};

use crate::audio_bank_blend_association;
use crate::audio_bank_blend_origins::{self, Occurrence};
use crate::audio_bank_blend_payload::{BlendLayerRecord, BlendPayload, LAYER_NAMESPACE};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_origins::NodeSites;
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Subresource};

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    sites: &NodeSites,
    mut layer: BankBlendLayer<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    emit(projection, sites, &layer, writer)?;
    let id_range = &layer.fields.layer_id.range;
    let occurrence = Occurrence::new(layer.ordinal, id_range);
    let owner = RtpcOwner::blend_layer(layer.ordinal, id_range, layer.checked_range.end);
    let issue = layer.issue.clone();
    let read = audio_bank_rtpc::node_rows(layer.rtpc(), issue)?;
    audio_bank_rtpc::project(projection, version, original, owner, read, writer)?;
    if let Some(rows) = layer.associations() {
        audio_bank_blend_association::project(projection, sites, occurrence, rows, writer)?;
    }
    Ok(())
}

fn emit<R>(
    projection: &BankProjection<'_>,
    sites: &NodeSites,
    row: &BankBlendLayer<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let id_range = &row.fields.layer_id.range;
    let mut prepared = sites.record(projection.input, writer, Some(id_range))?;
    sites.index(&mut prepared, 2, id_range, "blendLayerPosition")?;
    let status = audio_bank_blend_origins::layout(
        sites,
        &mut prepared,
        &row.checked_range,
        row.issue.is_some(),
    )?;
    let record = read(sites, &mut prepared, &row.fields, row.issue.is_some())?;
    let issue = row.issue.as_ref().map(|error| {
        MediaIssue::new(
            "audio.bank.truncated-blend-layer",
            "HIRC",
            Severity::Error,
            error.to_string(),
        )
    });
    let [section, object] = sites.indices();
    prepared.finish().write(
        writer,
        &Subresource::new(
            LAYER_NAMESPACE,
            vec![section, object, u64::from(row.ordinal)],
        ),
        status,
        &BlendPayload::Layer(&record),
        issue.as_slice(),
    )
}

fn read(
    sites: &NodeSites,
    prepared: &mut BankOrigins<'_, '_, '_>,
    fields: &BankBlendLayerFields,
    malformed: bool,
) -> Result<BlendLayerRecord, MediaError> {
    sites.field(prepared, "layer_id", &fields.layer_id.range, "blendLayerId")?;
    let layer_id = (&fields.layer_id).into();
    let crossfade_rtpc_id = sites.scalar(
        prepared,
        "crossfade_rtpc_id",
        fields.crossfade_rtpc_id.as_ref(),
        "blendCrossfadeRtpcId",
    )?;
    let crossfade_rtpc_type = sites.scalar(
        prepared,
        "crossfade_rtpc_type",
        fields.crossfade_rtpc_type.as_ref(),
        "blendCrossfadeRtpcType",
    )?;
    let associations = audio_bank_blend_origins::table(
        sites,
        prepared,
        fields.associations.as_ref(),
        "associations",
        "blendAssociationCount",
    )?;
    match (
        malformed,
        crossfade_rtpc_id,
        crossfade_rtpc_type,
        associations,
    ) {
        (false, Some(crossfade_rtpc_id), Some(crossfade_rtpc_type), Some(associations)) => {
            Ok(BlendLayerRecord::Decoded {
                layer_id,
                crossfade_rtpc_id,
                crossfade_rtpc_type,
                associations,
            })
        }
        (true, crossfade_rtpc_id, crossfade_rtpc_type, associations) => {
            Ok(BlendLayerRecord::Malformed {
                layer_id,
                crossfade_rtpc_id,
                crossfade_rtpc_type,
                associations,
            })
        }
        _ => Err(MediaError::Contract(
            "complete Blend Layer lacks checked fields".into(),
        )),
    }
}
