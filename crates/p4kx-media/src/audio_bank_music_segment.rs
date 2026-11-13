// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read, Seek};
use std::path::Path;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::bank::{
    BankHircMusicSegment, BankMusicMarker, BankMusicSegmentRead, BankMusicSegmentRow,
    BankMusicSegmentRows,
};

use crate::audio_bank_inventory::VersionSite;
use crate::audio_bank_music_node;
use crate::audio_bank_music_stinger_schedule;
use crate::audio_bank_music_value::MusicStingerRecord;
use crate::audio_bank_projection::BankProjection;
use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource, error};

#[path = "audio_bank_music_segment_origins.rs"]
mod origins;
#[path = "audio_bank_music_segment_payload.rs"]
mod payload;

pub(crate) use self::origins::{MusicChildField, MusicSites};
pub(crate) use self::payload::MusicSegmentPayload;
use self::payload::{
    MusicChildRecord, MusicMarkerEncoding, MusicMarkerRecord, MusicSegmentLayout, SegmentRecord,
    entry_range,
};

pub(crate) const NAMESPACE: &str = "audio-bank-music-segment";
pub(crate) const CHILD: &str = "audio-bank-music-segment-child";
pub(crate) const STINGER: &str = "audio-bank-music-segment-stinger";
pub(crate) const MARKER: &str = "audio-bank-music-segment-marker";

pub(crate) fn project<R: Read + Seek>(
    projection: &BankProjection<'_>,
    version: VersionSite,
    mut segment: BankHircMusicSegment<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let object = segment.object;
    let error = match &segment.status {
        BankMusicSegmentRead::Decoded => Some(None),
        BankMusicSegmentRead::Malformed(error) => Some(Some(error.clone())),
        BankMusicSegmentRead::Unsupported(_) => None,
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
            segment.shared_node(),
            error.as_ref(),
            writer,
        )?;
    } else {
        audio_bank_music_node::unsupported(projection, version, &object, writer)?;
    }
    let sites = MusicSites::new(&segment.object, version, &segment.fields);
    let (layout, status, issue) = interpretation(&segment.status)?;
    let data = MusicSegmentPayload::Segment(SegmentRecord::read(&segment.fields, layout));
    sites
        .header(projection.input, issue.is_some(), writer)?
        .write(
            writer,
            &Subresource::new(NAMESPACE, sites.object_indices().to_vec()),
            status,
            &data,
            issue.as_slice(),
        )?;
    let mut rows = segment.rows;
    while let Some(row) = rows.next() {
        let row = row.map_err(|cause| projection.replay_failure(cause))?;
        emit(projection, &sites, row, &mut rows, writer)?;
    }
    audio_bank_music_stinger_schedule::tail(
        projection.input,
        &sites,
        segment.fields.node.stinger_tail.as_ref(),
        writer,
    )
}

fn emit<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &MusicSites,
    row: BankMusicSegmentRow,
    rows: &mut BankMusicSegmentRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    match row {
        BankMusicSegmentRow::Child { ordinal, id } => {
            let ordinal = u64::from(ordinal);
            let data = MusicSegmentPayload::Child(MusicChildRecord {
                ordinal: DecimalU64::new(ordinal),
                track_id: id.value,
                id_range: entry_range(&id.range),
            });
            sites
                .child(
                    projection.input,
                    ordinal,
                    &id.range,
                    MusicChildField::Track,
                    writer,
                )?
                .write(
                    writer,
                    &sites.subresource(CHILD, ordinal),
                    Status::Complete,
                    &data,
                    &[],
                )
        }
        BankMusicSegmentRow::Stinger { ordinal, value } => {
            let ordinal = u64::from(ordinal);
            audio_bank_music_stinger_schedule::complete(
                projection.input,
                sites,
                ordinal,
                &value,
                writer,
            )?;
            let data = MusicSegmentPayload::Stinger(MusicStingerRecord::new(ordinal, &value));
            sites
                .stinger(projection.input, ordinal, &value, writer)?
                .write(
                    writer,
                    &sites.subresource(STINGER, ordinal),
                    Status::Complete,
                    &data,
                    &[],
                )
        }
        BankMusicSegmentRow::Marker { ordinal, value } => {
            marker(projection, sites, u64::from(ordinal), &value, rows, writer)
        }
        _ => Err(MediaError::Contract("unexpected music segment row".into())),
    }
}

fn marker<R: Read + Seek>(
    projection: &BankProjection<'_>,
    sites: &MusicSites,
    ordinal: u64,
    value: &BankMusicMarker,
    rows: &mut BankMusicSegmentRows<'_, R>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let original = read_name(projection, value, rows)?;
    let name = String::from_utf8_lossy(&original);
    let encoding = if matches!(name, std::borrow::Cow::Borrowed(_)) {
        MusicMarkerEncoding::Utf8
    } else {
        MusicMarkerEncoding::InvalidUtf8
    };
    let data = MusicSegmentPayload::Marker(MusicMarkerRecord::new(ordinal, value, &name, encoding));
    sites
        .marker(projection.input, ordinal, value, writer)?
        .write(
            writer,
            &sites.subresource(MARKER, ordinal),
            Status::Complete,
            &data,
            &[],
        )
}

fn read_name<R: Read + Seek>(
    projection: &BankProjection<'_>,
    value: &BankMusicMarker,
    rows: &mut BankMusicSegmentRows<'_, R>,
) -> Result<Vec<u8>, MediaError> {
    let byte_length = value.name_range.end - value.name_range.start;
    let length = usize::try_from(byte_length)
        .map_err(|cause| error::io(Path::new(projection.path), io::Error::other(cause)))?;
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|cause| error::io(Path::new(projection.path), io::Error::other(cause)))?;
    bytes.resize(length, 0);
    rows.read_current_name(&mut bytes)
        .map_err(|cause| projection.replay_failure(cause))?;
    Ok(bytes)
}

fn interpretation(
    status: &BankMusicSegmentRead,
) -> Result<(MusicSegmentLayout, Status, Option<MediaIssue>), MediaError> {
    match status {
        BankMusicSegmentRead::Decoded => Ok((MusicSegmentLayout::Decoded, Status::Complete, None)),
        BankMusicSegmentRead::Malformed(error) => Ok((
            MusicSegmentLayout::Malformed,
            Status::Partial,
            Some(MediaIssue::new(
                "audio.bank.invalid-music-segment",
                "HIRC",
                Severity::Error,
                error.to_string(),
            )),
        )),
        BankMusicSegmentRead::Unsupported(error) => Ok((
            MusicSegmentLayout::Unsupported,
            Status::Complete,
            Some(MediaIssue::new(
                "audio.bank.unsupported-music-segment",
                "HIRC",
                Severity::Info,
                error.to_string(),
            )),
        )),
        _ => Err(MediaError::Contract(
            "unexpected MusicSegment read state".into(),
        )),
    }
}
