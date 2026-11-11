// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};

use p4kx_extraction::bank::{BankHircMusicPlaylist, BankMusicPlaylistRead, BankMusicPlaylistRow};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_segment::MusicChildField;
use crate::audio_bank_music_value::MusicStingerRecord;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

#[path = "audio_bank_music_playlist_origins.rs"]
mod origins;
#[path = "audio_bank_music_playlist_payload.rs"]
mod payload;

use self::origins::PlaylistSites;
pub(crate) use self::payload::MusicPlaylistPayload;
use self::payload::{PlaylistChildRecord, PlaylistItemRecord, PlaylistLayout, PlaylistRecord};

pub(crate) const NAMESPACE: &str = "audio-bank-music-playlist";
pub(crate) const CHILD: &str = "audio-bank-music-playlist-child";
pub(crate) const STINGER: &str = "audio-bank-music-playlist-stinger";
pub(crate) const ITEM: &str = "audio-bank-music-playlist-item";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    playlist: BankHircMusicPlaylist<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let sites = PlaylistSites::new(&playlist.object, version, &playlist.fields);
    let (layout, status, issue) = interpretation(&playlist.status)?;
    let payload = MusicPlaylistPayload::Playlist(PlaylistRecord::new(&playlist.fields, layout));
    sites
        .header(projection.input, issue.is_some(), writer)?
        .write(
            writer,
            &Subresource::new(NAMESPACE, sites.node().object_indices().to_vec()),
            status,
            &payload,
            issue.as_slice(),
        )?;
    for row in playlist.rows {
        let row = row.map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, &sites, row, writer)?;
    }
    Ok(())
}

fn emit(
    projection: &BankProjection<'_>,
    sites: &PlaylistSites<'_>,
    row: BankMusicPlaylistRow,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let input = projection.input;
    let (namespace, ordinal, data, prepared) = match row {
        BankMusicPlaylistRow::Child { ordinal, id } => {
            let ordinal = u64::from(ordinal);
            (
                CHILD,
                ordinal,
                MusicPlaylistPayload::Child(PlaylistChildRecord::new(ordinal, &id)),
                sites
                    .node()
                    .child(input, ordinal, &id.range, MusicChildField::Child, writer)?,
            )
        }
        BankMusicPlaylistRow::Stinger { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            (
                STINGER,
                ordinal,
                MusicPlaylistPayload::Stinger(MusicStingerRecord::new(ordinal, &value)),
                sites.node().stinger(input, ordinal, &value, writer)?,
            )
        }
        BankMusicPlaylistRow::Item { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            (
                ITEM,
                ordinal,
                MusicPlaylistPayload::Item(PlaylistItemRecord::new(ordinal, &value)),
                sites.item(input, ordinal, &value, writer)?,
            )
        }
        _ => return Err(MediaError::Contract("unexpected MusicPlaylist row".into())),
    };
    prepared.write(
        writer,
        &sites.node().subresource(namespace, ordinal),
        Status::Complete,
        &data,
        &[],
    )
}

fn interpretation(
    status: &BankMusicPlaylistRead,
) -> Result<(PlaylistLayout, Status, Option<MediaIssue>), MediaError> {
    match status {
        BankMusicPlaylistRead::Decoded => Ok((PlaylistLayout::Decoded, Status::Complete, None)),
        BankMusicPlaylistRead::Malformed(error) => Ok((
            PlaylistLayout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-music-playlist",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        )),
        BankMusicPlaylistRead::Unsupported(error) => Ok((
            PlaylistLayout::Unsupported,
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-music-playlist",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
        )),
        _ => Err(MediaError::Contract(
            "unexpected MusicPlaylist read state".into(),
        )),
    }
}
