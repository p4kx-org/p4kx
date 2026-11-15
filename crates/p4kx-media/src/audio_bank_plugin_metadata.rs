// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::BankError;
use p4kx_extraction::bank::{BankHircObject, BankHircPlugin, BankNodeFxKind, BankPluginParameters};

use crate::audio_bank_inline_state::{self, InlineStateRead};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_declarations::{Declarations, Disposition};
use crate::audio_bank_node_fx;
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_state::StateProjection;
use crate::{MediaError, MediaWriter, Status, Subresource};

use super::payload::{MetadataPayload, ObservedPlugin, PLUGIN, PluginParameters};

#[path = "audio_bank_plugin_metadata_rows.rs"]
mod rows;

pub(in crate::audio_bank_hirc) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    mut reader: BankHircPlugin<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = *reader.object();
    let sites = NodeSites::new(&original, version, NodeKind::Plugin);
    let mut origins = sites.record(bank.input, writer, None)?;
    let observed = observations(&sites, &mut origins, &reader, version.version)?;
    let error = reader.format_error().cloned();
    let (payload, status, issue) = super::declaration(
        version.version,
        error.as_ref(),
        observed,
        "audio.bank.invalid-hirc-plugin",
    );
    origins.finish().write(
        writer,
        &Subresource::new(PLUGIN, sites.indices().to_vec()),
        status,
        &MetadataPayload::Plugin(&payload),
        issue.as_slice(),
    )?;
    rows::project(bank, &sites, &mut reader, writer)?;
    let rtpc = audio_bank_rtpc::node_rows(reader.rtpc(), error.clone())?;
    audio_bank_rtpc::project(bank, version, &original, RtpcOwner::Plugin, rtpc, writer)?;
    let inline = reader
        .states()
        .map_or(InlineStateRead::Unavailable, InlineStateRead::Readable);
    let state_mapping = StateProjection::new(bank.input, &original, version);
    audio_bank_inline_state::project(bank, &state_mapping, inline, writer)?;
    device_slots(
        bank,
        version,
        &original,
        &mut reader,
        error.as_ref(),
        writer,
    )
}

fn observations<R>(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    reader: &BankHircPlugin<'_, R>,
    version: u32,
) -> Result<ObservedPlugin, MediaError> {
    let effect = reader.effect();
    let coverage = super::coverage(
        sites,
        origins,
        reader.object().kind(version),
        &reader.checked_range(),
        reader.unprojected_range().as_ref(),
        reader.format_error().is_some(),
    )?;
    Ok(ObservedPlugin {
        coverage,
        plugin_id: sites.scalar(
            origins,
            "observed/plugin_id",
            effect.plugin.plugin_id.as_ref(),
            "pluginId",
        )?,
        parameters: effect
            .plugin
            .parameters
            .as_ref()
            .map(|value| parameters(sites, origins, value))
            .transpose()?,
        bank_data_count: sites.scalar(
            origins,
            "observed/bank_data_count",
            effect.plugin.bank_data_count.as_ref(),
            "pluginBankDataCount",
        )?,
        override_count: sites.scalar(
            origins,
            "observed/override_count",
            effect.override_count.as_ref(),
            "pluginOverrideCount",
        )?,
    })
}

fn parameters(
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
    value: &BankPluginParameters,
) -> Result<PluginParameters, MediaError> {
    sites.field(
        origins,
        "observed/parameters/declared_size",
        &value.size.range,
        "pluginParametersSize",
    )?;
    sites.field(
        origins,
        "observed/parameters/range",
        &value.range,
        "pluginParametersBytes",
    )?;
    sites.converted(
        origins,
        "observed/parameters/complete",
        &(value.size.range.start..value.range.end),
        "pluginParametersCoverage",
    )?;
    Ok(PluginParameters {
        declared_size: (&value.size).into(),
        range: (&value.range).into(),
        complete: value.is_complete(),
    })
}

fn device_slots<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    reader: &mut BankHircPlugin<'_, R>,
    error: Option<&BankError>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if let Some(slots) = reader.device_slots() {
        let declarations = Declarations::new(bank, original, version);
        let kind = BankNodeFxKind::Effects;
        audio_bank_node_fx::summary(
            &declarations,
            kind,
            Disposition::Observed(error),
            Some(slots.list()),
            writer,
        )?;
        if let Some(rows) = reader.effect_slots() {
            audio_bank_node_fx::slots(&declarations, kind, rows, writer)?;
        }
    }
    Ok(())
}

pub(in crate::audio_bank_hirc) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    original: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(original, version, NodeKind::Plugin);
    let mut origins = sites.record(bank.input, writer, None)?;
    sites.summary(&mut origins, None, false)?;
    origins.finish().write(
        writer,
        &Subresource::new(PLUGIN, sites.indices().to_vec()),
        Status::Complete,
        &MetadataPayload::Plugin(&NodeDeclaration::Unsupported {
            version: version.version,
        }),
        &[],
    )
}
