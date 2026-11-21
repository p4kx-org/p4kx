// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankHircMusicTrack, BankMusicTrackRead, BankMusicTrackRow};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_node;
use crate::audio_bank_music_track_controls;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status};

#[path = "audio_bank_music_track_origins.rs"]
mod origins;
#[path = "audio_bank_music_track_payload.rs"]
mod payload;

use self::origins::{List, TrackSites};
pub(crate) use self::payload::MusicTrackPayload;
use self::payload::{
    TrackClipRecord, TrackLayout, TrackRecord, TrackSourceRecord, TrackSwitchMember,
};

pub(crate) const NAMESPACE: &str = "audio-bank-music-track";
pub(crate) const SOURCE: &str = "audio-bank-music-track-source";
pub(crate) const CLIP: &str = "audio-bank-music-track-clip";
pub(crate) const SWITCH_MEMBER: &str = "audio-bank-music-track-switch-member";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    mut track: BankHircMusicTrack<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let object = track.object;
    let error = match &track.status {
        BankMusicTrackRead::Decoded => Some(None),
        BankMusicTrackRead::Malformed(error) => Some(Some(error.clone())),
        BankMusicTrackRead::Unsupported(_) => None,
        _ => {
            return Err(MediaError::Contract(
                "unexpected shared music node disposition".into(),
            ));
        }
    };
    if let Some(error) = error {
        audio_bank_music_node::project(
            projection,
            version,
            object,
            track.shared_node(),
            error.as_ref(),
            writer,
        )?;
    } else {
        audio_bank_music_node::unsupported(projection, version, &object, writer)?;
    }
    let sites = TrackSites::new(&track.object, version, &track.fields);
    let (layout, status, issue) = interpretation(&track.status)?;
    let data = MusicTrackPayload::Track(TrackRecord::new(&track.fields, layout));
    sites
        .header(projection.input, issue.is_some(), writer)?
        .write(
            writer,
            &sites.subresource(None),
            status,
            &data,
            issue.as_slice(),
        )?;
    for row in &mut track.rows {
        let row = row.map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, &sites, row, writer)?;
    }
    audio_bank_music_track_controls::project(projection, version, &mut track, writer)
}

fn emit(
    projection: &BankProjection<'_>,
    sites: &TrackSites<'_>,
    row: BankMusicTrackRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let input = projection.input;
    let (list, ordinal, data, prepared) = match row {
        BankMusicTrackRow::Source { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            (
                List::Source,
                ordinal,
                MusicTrackPayload::Source(TrackSourceRecord::new(ordinal, &value)?),
                sites.source(input, ordinal, &value, writer)?,
            )
        }
        BankMusicTrackRow::Clip { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            (
                List::Clip,
                ordinal,
                MusicTrackPayload::Clip(TrackClipRecord::new(ordinal, &value)),
                sites.clip(input, ordinal, &value, writer)?,
            )
        }
        BankMusicTrackRow::Switch { ordinal, id } => {
            let ordinal = u64::from(ordinal);
            (
                List::SwitchMember,
                ordinal,
                MusicTrackPayload::SwitchMember(TrackSwitchMember::new(ordinal, &id)),
                sites.switch_member(input, ordinal, &id, writer)?,
            )
        }
        _ => return Err(MediaError::Contract("unexpected MusicTrack row".into())),
    };
    prepared.write(
        writer,
        &sites.subresource(Some((list, ordinal))),
        Status::Complete,
        &data,
        &[],
    )
}

fn interpretation(
    status: &BankMusicTrackRead,
) -> Result<(TrackLayout, Status, Option<MediaIssue>), MediaError> {
    match status {
        BankMusicTrackRead::Decoded => Ok((TrackLayout::Decoded, Status::Complete, None)),
        BankMusicTrackRead::Malformed(error) => Ok((
            TrackLayout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-music-track",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        )),
        BankMusicTrackRead::Unsupported(error) => Ok((
            TrackLayout::Unsupported,
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-music-track",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
        )),
        _ => Err(MediaError::Contract(
            "unexpected MusicTrack read state".into(),
        )),
    }
}
