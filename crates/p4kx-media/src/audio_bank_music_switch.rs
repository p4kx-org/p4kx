// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{
    BankHircMusicSwitch, BankMusicSwitchRead, BankMusicSwitchRow, BankMusicSwitchRows,
    BankMusicSwitchTarget,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_segment::MusicChildField;
use crate::audio_bank_music_stinger_schedule;
use crate::audio_bank_music_value::{MusicNodeChildRecord, MusicStingerRecord};
use crate::audio_bank_projection::BankProjection;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_music_switch_origins.rs"]
mod origins;
#[path = "audio_bank_music_switch_payload.rs"]
mod payload;

use self::origins::SwitchSites;
pub(crate) use self::payload::MusicSwitchPayload;
use self::payload::{
    SwitchGroupRecord, SwitchLayout, SwitchNodeRecord, SwitchRecord, SwitchTargetRecord,
};

pub(crate) const NAMESPACE: &str = "audio-bank-music-switch";
pub(crate) const CHILD: &str = "audio-bank-music-switch-child";
pub(crate) const STINGER: &str = "audio-bank-music-switch-stinger";
pub(crate) const GROUP: &str = "audio-bank-music-switch-group";
pub(crate) const NODE: &str = "audio-bank-music-switch-node";
pub(crate) const TARGET: &str = "audio-bank-music-switch-target";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    switch: BankHircMusicSwitch<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = SwitchSites::new(&switch.object, version, &switch.fields);
    let (layout, status, issue) = interpretation(&switch.status)?;
    let payload = MusicSwitchPayload::Switch(Box::new(SwitchRecord::new(&switch.fields, layout)));
    sites
        .header(projection.input, issue.is_some(), writer)?
        .write(
            writer,
            &Subresource::new(NAMESPACE, sites.node().object_indices().to_vec()),
            status,
            &payload,
            issue.as_slice(),
        )?;
    let mut rows = switch.rows;
    while let Some(row) = rows.next() {
        let row = row.map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, &sites, row, &rows, writer)?;
    }
    audio_bank_music_stinger_schedule::tail(
        projection.input,
        sites.node(),
        switch.fields.node.stinger_tail.as_ref(),
        writer,
    )
}

fn emit<R>(
    projection: &BankProjection<'_>,
    sites: &SwitchSites<'_>,
    row: BankMusicSwitchRow,
    rows: &BankMusicSwitchRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let input = projection.input;
    let (namespace, ordinal, data, prepared) = match row {
        BankMusicSwitchRow::Child { ordinal, id } => {
            let ordinal = u64::from(ordinal);
            let data = MusicSwitchPayload::Child(MusicNodeChildRecord::new(ordinal, &id));
            let origins =
                sites
                    .node()
                    .child(input, ordinal, &id.range, MusicChildField::Child, writer)?;
            (CHILD, ordinal, data, origins)
        }
        BankMusicSwitchRow::Stinger { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            audio_bank_music_stinger_schedule::complete(
                input,
                sites.node(),
                ordinal,
                &value,
                writer,
            )?;
            let data = MusicSwitchPayload::Stinger(MusicStingerRecord::new(ordinal, &value));
            let origins = sites.node().stinger(input, ordinal, &value, writer)?;
            (STINGER, ordinal, data, origins)
        }
        BankMusicSwitchRow::Group { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            let data = MusicSwitchPayload::Group(SwitchGroupRecord::new(ordinal, &value));
            let origins = sites.group(input, ordinal, &value, writer)?;
            (GROUP, ordinal, data, origins)
        }
        BankMusicSwitchRow::Node { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            let data = MusicSwitchPayload::Node(SwitchNodeRecord::new(ordinal, &value));
            let origins = sites.physical_node(input, ordinal, &value, writer)?;
            (NODE, ordinal, data, origins)
        }
        BankMusicSwitchRow::Target { ordinal, value } => {
            return emit_target(projection, sites, u64::from(ordinal), &value, rows, writer);
        }
        _ => return Err(MediaError::Contract("unexpected MusicSwitch row".into())),
    };
    write_row(sites, namespace, ordinal, &data, prepared, writer)
}

fn emit_target<R>(
    projection: &BankProjection<'_>,
    sites: &SwitchSites<'_>,
    ordinal: u64,
    target: &BankMusicSwitchTarget,
    rows: &BankMusicSwitchRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (prepared, ancestors) = sites.target(
        projection.input,
        ordinal,
        target,
        rows.current_target_route(),
        writer,
    )?;
    let data = MusicSwitchPayload::Target(SwitchTargetRecord::new(ordinal, target, ancestors));
    write_row(sites, TARGET, ordinal, &data, prepared, writer)
}

fn write_row(
    sites: &SwitchSites<'_>,
    namespace: &'static str,
    ordinal: u64,
    data: &MusicSwitchPayload,
    prepared: PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    prepared.write(
        writer,
        &sites.node().subresource(namespace, ordinal),
        Status::Complete,
        data,
        &[],
    )
}

fn interpretation(
    status: &BankMusicSwitchRead,
) -> Result<(SwitchLayout, Status, Option<MediaIssue>), MediaError> {
    match status {
        BankMusicSwitchRead::Decoded => Ok((SwitchLayout::Decoded, Status::Complete, None)),
        BankMusicSwitchRead::Malformed(error) => Ok((
            SwitchLayout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-music-switch",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        )),
        BankMusicSwitchRead::Unsupported(error) => Ok((
            SwitchLayout::Unsupported,
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-music-switch",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
        )),
        _ => Err(MediaError::Contract(
            "unexpected MusicSwitch read state".into(),
        )),
    }
}
