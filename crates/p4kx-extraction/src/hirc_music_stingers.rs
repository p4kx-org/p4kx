// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::ops::Range;

use p4kx_audio::{BankHircField, BankMusicStinger, BankMusicStingerTail, BankReadError};
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::hirc_view::StateRange;
use crate::source_evidence_error::SourceEvidenceError;
use crate::source_value::SourcedValue;

use super::super::origins::ObjectOrigins;

pub(super) enum Observation {
    Skip,
    Stinger {
        ordinal: u32,
        value: BankMusicStinger,
    },
    End,
}

pub(super) struct Stingers<'view, I, F> {
    rows: RefCell<I>,
    select: F,
    tail: Option<BankMusicStingerTail>,
    origins: &'view ObjectOrigins,
}

impl<'view, I, F> Stingers<'view, I, F> {
    pub(super) const fn new(
        rows: I,
        select: F,
        tail: Option<BankMusicStingerTail>,
        origins: &'view ObjectOrigins,
    ) -> Self {
        Self {
            rows: RefCell::new(rows),
            select,
            tail,
            origins,
        }
    }
}

impl<I, F, Row> Serialize for Stingers<'_, I, F>
where
    I: Iterator<Item = Result<Row, BankReadError>>,
    F: Fn(Row) -> Result<Observation, &'static str>,
{
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut rows = self.rows.try_borrow_mut().map_err(S::Error::custom)?;
        let mut output = serializer.serialize_seq(None)?;
        for row in &mut *rows {
            match (self.select)(row.map_err(S::Error::custom)?).map_err(S::Error::custom)? {
                Observation::Skip => {}
                Observation::Stinger { ordinal, value } => {
                    let row = Stinger::complete(self.origins, ordinal, &value)
                        .map_err(S::Error::custom)?;
                    output.serialize_element(&row)?;
                }
                Observation::End => break,
            }
        }
        if let Some(tail) = &self.tail {
            let row = Stinger::partial(self.origins, tail).map_err(S::Error::custom)?;
            output.serialize_element(&row)?;
        }
        output.end()
    }
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub(in crate::hirc_view) struct Stinger {
    ordinal: SourcedValue<u32>,
    checked_range: SourcedValue<StateRange>,
    complete: SourcedValue<bool>,
    trigger_id: SourcedValue<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segment_id: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sync_play_at: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cue_filter_hash: Option<SourcedValue<u32>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    dont_repeat_time: Option<RepeatTime>,
    #[serde(skip_serializing_if = "Option::is_none")]
    segment_look_ahead: Option<SourcedValue<u32>>,
}

#[derive(Serialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
struct RepeatTime {
    bits: SourcedValue<u32>,
    signed_value: SourcedValue<i32>,
}

impl Stinger {
    fn complete(
        origins: &ObjectOrigins,
        ordinal: u32,
        original: &BankMusicStinger,
    ) -> Result<Self, SourceEvidenceError> {
        Self::observe(
            origins,
            ordinal,
            &(original.trigger_id.range.start..original.opaque_range.end),
            &original.trigger_id,
            [
                Some(&original.segment_id),
                Some(&original.sync_play_at),
                Some(&original.cue_filter_hash),
                Some(&original.dont_repeat_time_bits),
                Some(&original.segment_look_ahead),
            ],
        )
    }

    fn partial(
        origins: &ObjectOrigins,
        original: &BankMusicStingerTail,
    ) -> Result<Self, SourceEvidenceError> {
        Self::observe(
            origins,
            original.ordinal,
            &original.checked_range,
            &original.trigger_id,
            [
                original.segment_id.as_ref(),
                original.sync_play_at.as_ref(),
                original.cue_filter_hash.as_ref(),
                original.dont_repeat_time_bits.as_ref(),
                None,
            ],
        )
    }

    fn observe(
        origins: &ObjectOrigins,
        ordinal: u32,
        range: &Range<u64>,
        trigger: &BankHircField<u32>,
        [segment, sync, cue, repeat, ahead]: [Option<&BankHircField<u32>>; 5],
    ) -> Result<Self, SourceEvidenceError> {
        Ok(Self {
            ordinal: origins.derived(ordinal, range, "musicStingerOrdinal")?,
            checked_range: origins.range(range, "musicStingerCheckedRange")?,
            complete: origins.derived(ahead.is_some(), range, "musicStingerCoverage")?,
            trigger_id: origins.field(trigger, "musicStingerTriggerId")?,
            segment_id: origins.optional(segment, "musicStingerSegmentId")?,
            sync_play_at: origins.optional(sync, "musicStingerSyncPlayAt")?,
            cue_filter_hash: origins.optional(cue, "musicStingerCueFilterHash")?,
            dont_repeat_time: repeat.map(|word| repeat_time(origins, word)).transpose()?,
            segment_look_ahead: origins.optional(ahead, "musicStingerSegmentLookAhead")?,
        })
    }
}

fn repeat_time(
    origins: &ObjectOrigins,
    word: &BankHircField<u32>,
) -> Result<RepeatTime, SourceEvidenceError> {
    let label = "musicStingerDontRepeatTime";
    Ok(RepeatTime {
        bits: origins.field(word, label)?,
        signed_value: origins.derived(
            i32::from_le_bytes(word.value.to_le_bytes()),
            &word.range,
            label,
        )?,
    })
}
