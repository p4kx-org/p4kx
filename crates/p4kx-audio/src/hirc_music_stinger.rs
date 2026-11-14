// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

const WORD_COUNT: usize = 6;
pub(crate) const STINGER_LEN: usize = WORD_COUNT * size_of::<u32>();

/// Original fields of a complete version-154 music stinger.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankMusicStinger {
    /// Original trigger ID.
    pub trigger_id: BankHircField<u32>,
    /// Original segment ID.
    pub segment_id: BankHircField<u32>,
    /// Original synchronization code.
    pub sync_play_at: BankHircField<u32>,
    /// Original cue filter hash.
    pub cue_filter_hash: BankHircField<u32>,
    /// Original signed repeat-time bits.
    pub dont_repeat_time_bits: BankHircField<u32>,
    /// Original segment look-ahead count.
    pub segment_look_ahead: BankHircField<u32>,
    /// Original extent after the IDs.
    pub opaque_range: Range<u64>,
}

/// Readable words of the final truncated version-154 music stinger.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicStingerTail {
    /// Zero-based position in the original stinger table.
    pub ordinal: u32,
    /// Checked original row bytes including any incomplete last word.
    pub checked_range: Range<u64>,
    /// Original trigger ID.
    pub trigger_id: BankHircField<u32>,
    /// Original segment ID when readable.
    pub segment_id: Option<BankHircField<u32>>,
    /// Original synchronization code when readable.
    pub sync_play_at: Option<BankHircField<u32>>,
    /// Original cue filter hash when readable.
    pub cue_filter_hash: Option<BankHircField<u32>>,
    /// Original signed repeat-time bits when readable.
    pub dont_repeat_time_bits: Option<BankHircField<u32>>,
}

pub(crate) fn stinger<F: ObjectFields>(reader: &mut F) -> Result<BankMusicStinger, F::Error> {
    let mut offset = reader.offset();
    let bytes = reader.read_array::<STINGER_LEN>()?;
    let [trigger, segment, sync, cue, repeat, ahead] = bytes.as_chunks::<{ size_of::<u32>() }>().0
    else {
        unreachable!("a stinger contains six original 32-bit words");
    };
    let [
        trigger_id,
        segment_id,
        sync_play_at,
        cue_filter_hash,
        dont_repeat_time_bits,
        segment_look_ahead,
    ] = [trigger, segment, sync, cue, repeat, ahead].map(|word| {
        let start = offset;
        offset += size_of::<u32>() as u64;
        BankHircField {
            value: u32::from_le_bytes(*word),
            range: start..offset,
        }
    });
    let opaque_range = segment_id.range.end..reader.offset();
    Ok(BankMusicStinger {
        trigger_id,
        segment_id,
        sync_play_at,
        cue_filter_hash,
        dont_repeat_time_bits,
        segment_look_ahead,
        opaque_range,
    })
}

pub(crate) fn tail<F: ObjectFields>(
    reader: &mut F,
    ordinal: u32,
    observed: &mut Option<BankMusicStingerTail>,
) -> Result<(), F::Error> {
    let start = reader.offset();
    let end = start + reader.remaining_bytes();
    let mut words = [const { None }; WORD_COUNT];
    let result = read_words(reader, &mut words);
    let [
        trigger_id,
        segment_id,
        sync_play_at,
        cue_filter_hash,
        dont_repeat_time_bits,
        _,
    ] = words;
    if let Some(trigger_id) = trigger_id {
        *observed = Some(BankMusicStingerTail {
            ordinal,
            checked_range: start..end,
            trigger_id,
            segment_id,
            sync_play_at,
            cue_filter_hash,
            dont_repeat_time_bits,
        });
    }
    result
}

fn read_words<F: ObjectFields>(
    reader: &mut F,
    words: &mut [Option<BankHircField<u32>>; WORD_COUNT],
) -> Result<(), F::Error> {
    for word in words {
        *word = Some(reader.read_u32()?);
    }
    Ok(())
}
