// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankBusDuckRow, BankHircBus, BankHircObject, BankNodePropertyKind};

use crate::audio_bank_inline_state::{self, InlineStateRead};
use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_node_declaration_payload::NodeDeclaration;
use crate::audio_bank_node_declarations::{self, Owner};
use crate::audio_bank_node_origins::{NodeKind, NodeSites};
use crate::audio_bank_node_property_rows::{self, PropertyRows, PropertyScope};
use crate::audio_bank_origins::BankOrigins;
use crate::audio_bank_payload::BankEntryRange;
use crate::audio_bank_projection::BankProjection;
use crate::audio_bank_rtpc;
use crate::audio_bank_rtpc_payload::RtpcOwner;
use crate::audio_bank_state::StateProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_bus_payload.rs"]
pub(super) mod payload;

use payload::{BusObservations, BusPayload, DUCK, DuckRecord, SUMMARY};

pub(crate) fn project<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    mut bus: BankHircBus<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let object = *bus.object();
    let sites = NodeSites::new(&object, version, NodeKind::Bus);
    let mut origins = sites.record(bank.input, writer, None)?;
    let observed = observations(&bus, &sites, &mut origins)?;
    let issue = bus.format_error().map(|error| {
        MediaIssue::new(
            "audio.bank.invalid-bus",
            "HIRC",
            Severity::Error,
            error.to_string(),
        )
    });
    let (layout, status) = if issue.is_some() {
        (
            NodeDeclaration::Malformed {
                version: version.version,
                observed,
            },
            Status::Partial,
        )
    } else {
        (
            NodeDeclaration::Decoded {
                version: version.version,
                observed,
            },
            Status::Complete,
        )
    };
    sites.summary(
        &mut origins,
        Some(&(object.body_offset()..object.body_end_exclusive())),
        issue.is_some(),
    )?;
    origins.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        status,
        &BusPayload::Bus(&layout),
        issue.as_slice(),
    )?;
    project_rows(bank, version, &object, &sites, &mut bus, writer)
}

pub(crate) fn unsupported(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = NodeSites::new(object, version, NodeKind::Bus);
    let mut origins = sites.record(bank.input, writer, None)?;
    sites.summary(&mut origins, None, false)?;
    let layout = NodeDeclaration::Unsupported {
        version: version.version,
    };
    origins.finish().write(
        writer,
        &Subresource::new(SUMMARY, sites.indices().to_vec()),
        Status::Complete,
        &BusPayload::Bus(&layout),
        &[],
    )
}

fn project_rows<R: Read + Seek>(
    bank: &BankProjection<'_>,
    version: VersionSite,
    object: &BankHircObject,
    sites: &NodeSites,
    bus: &mut BankHircBus<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if let Some(rows) = bus.ducks() {
        for row in rows {
            let row = row.map_err(|cause| bank.replay_failure(cause))?;
            duck(bank, sites, &row, writer)?;
        }
    }
    if let Some(rows) = bus.properties() {
        let properties = PropertyRows::new(bank, sites, PropertyScope::Node);
        audio_bank_node_property_rows::emit(
            &properties,
            BankNodePropertyKind::Properties,
            rows,
            writer,
        )?;
    }
    audio_bank_node_declarations::emit(bank, version, Owner::Bus(bus), writer)?;
    let state = StateProjection::new(bank.input, object, version);
    let inline = bus
        .states()
        .map_or(InlineStateRead::Unavailable, InlineStateRead::Readable);
    audio_bank_inline_state::project(bank, &state, inline, writer)?;
    let error = bus.format_error().cloned();
    let read = audio_bank_rtpc::node_rows(bus.rtpc(), error)?;
    audio_bank_rtpc::project(bank, version, object, RtpcOwner::Bus, read, writer)
}

fn observations<R>(
    bus: &BankHircBus<'_, R>,
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
) -> Result<BusObservations, MediaError> {
    let original = bus.fields();
    let settings = &original.settings;

    Ok(BusObservations {
        override_bus_id: sites.scalar(
            origins,
            "observed/override_bus_id",
            original.override_bus_id.as_ref(),
            "busOverrideId",
        )?,
        device_shareset_id: sites.scalar(
            origins,
            "observed/device_shareset_id",
            original.device_shareset_id.as_ref(),
            "busDeviceSharesetId",
        )?,
        playback_flags: sites.scalar(
            origins,
            "observed/playback_flags",
            settings.playback_flags.as_ref(),
            "busPlaybackFlags",
        )?,
        max_instances: sites.scalar(
            origins,
            "observed/max_instances",
            settings.max_instances.as_ref(),
            "busMaxInstances",
        )?,
        channel_config_bits: sites.scalar(
            origins,
            "observed/channel_config_bits",
            settings.channel_config_bits.as_ref(),
            "busChannelConfigBits",
        )?,
        hdr_flags: sites.scalar(
            origins,
            "observed/hdr_flags",
            settings.hdr_flags.as_ref(),
            "busHdrFlags",
        )?,
        recovery_time: sites.scalar(
            origins,
            "observed/recovery_time",
            original.recovery_time.as_ref(),
            "busRecoveryTime",
        )?,
        max_duck_volume_bits: sites.scalar(
            origins,
            "observed/max_duck_volume_bits",
            original.max_duck_volume_bits.as_ref(),
            "busMaxDuckVolumeBits",
        )?,
        duck_count: sites.scalar(
            origins,
            "observed/duck_count",
            original.duck_count.as_ref(),
            "busDuckCount",
        )?,
        raw_remainder: raw_remainder(bus, sites, origins)?,
    })
}

fn raw_remainder<R>(
    bus: &BankHircBus<'_, R>,
    sites: &NodeSites,
    origins: &mut BankOrigins<'_, '_, '_>,
) -> Result<Option<BankEntryRange>, MediaError> {
    let raw = bus.raw_remainder();
    if raw.is_empty() {
        return Ok(None);
    }
    sites.range(origins, "observed/raw_remainder", &raw)?;
    Ok(Some((&raw).into()))
}

fn duck(
    bank: &BankProjection<'_>,
    sites: &NodeSites,
    row: &BankBusDuckRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut origins = sites.record(bank.input, writer, Some(&row.bus_id.range))?;
    sites.index(&mut origins, 2, &row.bus_id.range, "busDuckPosition")?;
    sites.field(&mut origins, "bus_id", &row.bus_id.range, "busDuckTargetId")?;
    sites.field(
        &mut origins,
        "volume_bits",
        &row.volume_bits.range,
        "busDuckVolumeBits",
    )?;
    sites.field(
        &mut origins,
        "fade_out_time",
        &row.fade_out_time.range,
        "busDuckFadeOutTime",
    )?;
    sites.field(
        &mut origins,
        "fade_in_time",
        &row.fade_in_time.range,
        "busDuckFadeInTime",
    )?;
    sites.field(
        &mut origins,
        "fade_curve",
        &row.fade_curve.range,
        "busDuckFadeCurve",
    )?;
    sites.field(
        &mut origins,
        "target_property",
        &row.target_property.range,
        "busDuckTargetProperty",
    )?;
    let mut indices = sites.indices().to_vec();
    indices.push(u64::from(row.ordinal));
    origins.finish().write(
        writer,
        &Subresource::new(DUCK, indices),
        Status::Complete,
        &BusPayload::Duck(&DuckRecord {
            bus_id: (&row.bus_id).into(),
            volume_bits: (&row.volume_bits).into(),
            fade_out_time: (&row.fade_out_time).into(),
            fade_in_time: (&row.fade_in_time).into(),
            fade_curve: (&row.fade_curve).into(),
            target_property: (&row.target_property).into(),
        }),
        &[],
    )
}
