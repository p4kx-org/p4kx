// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::io::{Read, Seek};

use p4kx_audio::{
    BankHircField, BankHircMusicTrack, BankMusicTrackClip, BankMusicTrackRow, BankMusicTrackRows,
    BankMusicTrackSource,
};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::source_evidence_error::SourceEvidenceError;

#[path = "hirc_music_track_automation_source_rows.rs"]
mod automation;

use super::super::origins::ObjectOrigins;
use super::dto::{Binary64Word, MusicTrackBinary64Value, MusicTrackSourceRow};

pub(super) struct MusicTrackRows<'view, 'input, R> {
    track: RefCell<BankHircMusicTrack<'input, R>>,
    origins: &'view ObjectOrigins,
}

impl<'view, 'input, R> MusicTrackRows<'view, 'input, R> {
    pub(super) const fn new(
        track: BankHircMusicTrack<'input, R>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self {
            track: RefCell::new(track),
            origins,
        }
    }
}

impl<R: Read + Seek> Serialize for MusicTrackRows<'_, '_, R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut track = self.track.try_borrow_mut().map_err(S::Error::custom)?;
        let mut sequence = serializer.serialize_seq(None)?;
        let first_member =
            serialize_sources_and_clips(&mut sequence, self.origins, &mut track.rows)?;
        if let Some(rows) = track.automations() {
            automation::serialize(&mut sequence, self.origins, rows)?;
        }
        for row in first_member.into_iter().map(Ok).chain(&mut track.rows) {
            let BankMusicTrackRow::Switch { ordinal, id } = row.map_err(S::Error::custom)? else {
                return Err(S::Error::custom(
                    "expected an original MusicTrack switch member",
                ));
            };
            let member = member(self.origins, ordinal, &id).map_err(S::Error::custom)?;
            sequence.serialize_element(&member)?;
        }
        sequence.end()
    }
}

fn serialize_sources_and_clips<R: Read + Seek, S: SerializeSeq>(
    sequence: &mut S,
    origins: &ObjectOrigins,
    rows: &mut BankMusicTrackRows<'_, R>,
) -> Result<Option<BankMusicTrackRow>, S::Error> {
    for row in rows {
        let projected = match row.map_err(S::Error::custom)? {
            BankMusicTrackRow::Source { ordinal, value } => media_source(origins, ordinal, &value),
            BankMusicTrackRow::Clip { ordinal, value } => clip(origins, ordinal, &value),
            member @ BankMusicTrackRow::Switch { .. } => return Ok(Some(member)),
            _ => return Err(S::Error::custom("unexpected original MusicTrack row")),
        }
        .map_err(S::Error::custom)?;
        sequence.serialize_element(&projected)?;
    }
    Ok(None)
}

fn media_source(
    origins: &ObjectOrigins,
    ordinal: u32,
    original: &BankMusicTrackSource,
) -> Result<MusicTrackSourceRow, SourceEvidenceError> {
    let fields = &original.fields;
    let plugin_parameters_range = original
        .plugin_parameters
        .as_ref()
        .map(|range| origins.range(range, "musicTrackPluginParametersExtent"))
        .transpose()?;
    Ok(MusicTrackSourceRow::MediaSource {
        ordinal: origins.derived(ordinal, &original.range, "musicTrackSourcePosition")?,
        range: origins.range(&original.range, "musicTrackSourceExtent")?,
        plugin_id: origins.optional(fields.plugin_id.as_ref(), "musicTrackPluginId")?,
        storage_code: origins.optional(fields.storage_code.as_ref(), "musicTrackStorageCode")?,
        source_id: origins.optional(fields.source_id.as_ref(), "musicTrackSourceId")?,
        cache_id: origins.optional(fields.cache_id.as_ref(), "musicTrackCacheId")?,
        in_memory_size: origins
            .optional(fields.in_memory_size.as_ref(), "musicTrackInMemorySize")?,
        source_bits: origins.optional(fields.source_bits.as_ref(), "musicTrackSourceBits")?,
        plugin_parameter_length: origins.optional(
            original.plugin_parameter_length.as_ref(),
            "musicTrackPluginParameterLength",
        )?,
        plugin_parameters_range,
    })
}

fn clip(
    origins: &ObjectOrigins,
    ordinal: u32,
    original: &BankMusicTrackClip,
) -> Result<MusicTrackSourceRow, SourceEvidenceError> {
    Ok(MusicTrackSourceRow::Clip {
        ordinal: origins.derived(ordinal, &original.track_id.range, "musicTrackClipPosition")?,
        range: origins.range(&original.range, "musicTrackClipExtent")?,
        track_id: origins.field(&original.track_id, "musicTrackClipTrackId")?,
        source_id: origins.field(&original.source_id, "musicTrackClipSourceId")?,
        cache_id: origins.optional(original.cache_id.as_ref(), "musicTrackClipCacheId")?,
        event_id: origins.field(&original.event_id, "musicTrackClipEventId")?,
        play_at: binary64(origins, &original.play_at, "musicTrackClipPlayAt")?,
        begin_trim: binary64(origins, &original.begin_trim, "musicTrackClipBeginTrim")?,
        end_trim: binary64(origins, &original.end_trim, "musicTrackClipEndTrim")?,
        source_duration: binary64(
            origins,
            &original.source_duration,
            "musicTrackClipSourceDuration",
        )?,
    })
}

fn member(
    origins: &ObjectOrigins,
    ordinal: u32,
    id: &BankHircField<u32>,
) -> Result<MusicTrackSourceRow, SourceEvidenceError> {
    Ok(MusicTrackSourceRow::SwitchMember {
        ordinal: origins.derived(ordinal, &id.range, "musicTrackSwitchMemberPosition")?,
        value_id: origins.field(id, "musicTrackSwitchValueId")?,
    })
}

fn binary64(
    origins: &ObjectOrigins,
    original: &BankHircField<u64>,
    label: &'static str,
) -> Result<MusicTrackBinary64Value, SourceEvidenceError> {
    let value = f64::from_bits(original.value);
    let finite_value = value
        .is_finite()
        .then(|| origins.derived(value, &original.range, label))
        .transpose()?;
    Ok(MusicTrackBinary64Value {
        bits: origins.field(original, label)?.map(Binary64Word),
        finite_value,
    })
}
