// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc::{self, SoundSource};
use crate::hirc_music::MusicClip;
use crate::hirc_source::{self, BankMediaSourceFields, SourceFlags};

/// One original `MusicTrack` source and its field ranges.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicTrackSource {
    /// Decoded media source.
    pub value: SoundSource,
    /// Original fixed source words.
    pub fields: BankMediaSourceFields,
    /// Original plug-in parameter length when present.
    pub plugin_parameter_length: Option<BankHircField<u32>>,
    /// Checked original plug-in parameter range.
    pub plugin_parameters: Option<Range<u64>>,
    /// Complete original source row range.
    pub range: Range<u64>,
}

/// One original `MusicTrack` clip.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicTrackClip {
    /// Original media source ID.
    pub source_id: BankHircField<u32>,
    /// Original cache ID when present.
    pub cache_id: Option<BankHircField<u32>>,
    /// Original event ID.
    pub event_id: BankHircField<u32>,
    /// Original play position as a binary64 word.
    pub play_at: BankHircField<u64>,
    /// Original start trim as a binary64 word.
    pub begin_trim: BankHircField<u64>,
    /// Original end trim as a binary64 word.
    pub end_trim: BankHircField<u64>,
    /// Original source duration as a binary64 word.
    pub source_duration: BankHircField<u64>,
    /// Complete original clip row range.
    pub range: Range<u64>,
}

impl BankMusicTrackClip {
    pub(super) const fn value(&self) -> MusicClip {
        MusicClip {
            source_id: self.source_id.value,
            event_id: self.event_id.value,
            play_at: f64::from_bits(self.play_at.value),
            begin_trim: f64::from_bits(self.begin_trim.value),
            end_trim: f64::from_bits(self.end_trim.value),
            source_duration: f64::from_bits(self.source_duration.value),
        }
    }
}

pub(crate) fn source<F: ObjectFields>(
    version: u32,
    input: &mut F,
) -> Result<BankMusicTrackSource, F::Error> {
    let start = input.offset();
    let mut fields = BankMediaSourceFields::default();
    let value = hirc_source::read_source(version, input, &mut fields, SourceFlags::Required)?;
    let mut plugin_parameter_length = None;
    let mut plugin_parameters = None;
    if value.is_source_plugin() {
        hirc_source::read_parameters(input, &mut plugin_parameter_length, &mut plugin_parameters)?;
    }
    Ok(BankMusicTrackSource {
        value,
        fields,
        plugin_parameter_length,
        plugin_parameters,
        range: start..input.offset(),
    })
}

pub(crate) fn clip<F: ObjectFields>(
    version: u32,
    input: &mut F,
) -> Result<BankMusicTrackClip, F::Error> {
    let start = input.offset();
    input.read_u32()?;
    let source_id = input.read_u32()?;
    let cache_id = if hirc::has_cache_id(version) {
        Some(input.read_u32()?)
    } else {
        None
    };
    Ok(BankMusicTrackClip {
        source_id,
        cache_id,
        event_id: input.read_u32()?,
        play_at: input.scalar(u64::from_le_bytes)?,
        begin_trim: input.scalar(u64::from_le_bytes)?,
        end_trim: input.scalar(u64::from_le_bytes)?,
        source_duration: input.scalar(u64::from_le_bytes)?,
        range: start..input.offset(),
    })
}
