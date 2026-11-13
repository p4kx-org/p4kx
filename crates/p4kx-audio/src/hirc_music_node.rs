// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::ObjectFields;
use crate::hirc_music::MusicMeter;
use crate::hirc_music_stinger::{self, BankMusicStingerTail};
use crate::hirc_node::{self, BankNodeParentFields};
use crate::{BankHircField, BankNodeParameters};

pub(crate) use crate::hirc_music_stinger::{BankMusicStinger, STINGER_LEN, stinger};

/// Original parent, child count, meter and stinger count of a music node.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicNodeFields {
    /// Original parent and the preceding fields that locate it.
    pub parent: BankNodeParentFields,
    /// Original checked groups following the parent declaration.
    pub parameters: BankNodeParameters,
    /// Traversed node base prefix, including checked opaque fields.
    pub node_range: Option<Range<u64>>,
    /// Original count of ordered child IDs.
    pub child_count: Option<BankHircField<u32>>,
    /// Admitted range of original child IDs.
    pub children_range: Option<Range<u64>>,
    /// Grid period stored as the exact binary64 word.
    pub grid_period: Option<BankHircField<u64>>,
    /// Grid offset stored as the exact binary64 word.
    pub grid_offset: Option<BankHircField<u64>>,
    /// Tempo stored as the exact binary32 word.
    pub tempo: Option<BankHircField<u32>>,
    /// Original beats per bar.
    pub beats_per_bar: Option<BankHircField<u8>>,
    /// Original note value of one beat.
    pub beat_value: Option<BankHircField<u8>>,
    /// Original stinger count.
    pub stinger_count: Option<BankHircField<u32>>,
    /// Readable original fields of a truncated final stinger when reached.
    pub stinger_tail: Option<BankMusicStingerTail>,
}

#[derive(Clone, Copy, Default)]
pub(crate) struct MusicList {
    pub(crate) start: u64,
    pub(crate) length: u64,
    pub(crate) count: u32,
}

impl MusicList {
    pub(crate) const fn record(&mut self, offset: u64) {
        self.length = offset - self.start;
        self.count += 1;
    }
}

#[derive(Default)]
pub(crate) struct MusicNodeLists {
    pub(crate) children: MusicList,
    pub(crate) stingers: MusicList,
}

pub(crate) fn scan<F: ObjectFields>(
    reader: &mut F,
    fields: &mut BankMusicNodeFields,
    lists: &mut MusicNodeLists,
) -> Result<(), F::Error> {
    let (_, count, length) = head(reader, fields)?;
    lists.children = MusicList {
        start: reader.offset(),
        count,
        length,
    };
    reader.skip_bytes(length)?;
    meter(reader, fields)?;
    let stingers = stinger_count(reader, fields)?;
    lists.stingers.start = reader.offset();
    for ordinal in 0..stingers {
        if reader.remaining_bytes() < STINGER_LEN as u64 {
            return hirc_music_stinger::tail(reader, ordinal, &mut fields.stinger_tail);
        }
        reader.skip_bytes(STINGER_LEN as u64)?;
        lists.stingers.record(reader.offset());
    }
    Ok(())
}

pub(crate) fn head<F: ObjectFields>(
    reader: &mut F,
    fields: &mut BankMusicNodeFields,
) -> Result<(u32, u32, u64), F::Error> {
    let start = reader.offset();
    reader.read_u8()?;
    let parent =
        hirc_node::read_base_parameters(reader, &mut fields.parent, &mut fields.parameters);
    fields.node_range = Some(start..reader.offset());
    let parent = parent?;
    let count = reader.read_u32()?;
    let children = count.value;
    fields.child_count = Some(count);
    let len = hirc_node::checked_child_bytes(children, reader.remaining_bytes())?;
    let start = reader.offset();
    fields.children_range = Some(start..start + len);
    Ok((parent, children, len))
}

pub(crate) fn meter<F: ObjectFields>(
    reader: &mut F,
    fields: &mut BankMusicNodeFields,
) -> Result<MusicMeter, F::Error> {
    let period = reader.scalar(u64::from_le_bytes)?;
    let grid_period = f64::from_bits(period.value);
    fields.grid_period = Some(period);
    let offset = reader.scalar(u64::from_le_bytes)?;
    let grid_offset = f64::from_bits(offset.value);
    fields.grid_offset = Some(offset);
    let tempo = reader.read_u32()?;
    let beats_per_minute = f32::from_bits(tempo.value);
    fields.tempo = Some(tempo);
    let beats = reader.read_u8()?;
    let beats_per_bar = beats.value;
    fields.beats_per_bar = Some(beats);
    let beat = reader.read_u8()?;
    let beat_value = beat.value;
    fields.beat_value = Some(beat);
    Ok(MusicMeter {
        grid_period,
        grid_offset,
        tempo: beats_per_minute,
        beats_per_bar,
        beat_value,
    })
}

pub(crate) fn stinger_count<F: ObjectFields>(
    reader: &mut F,
    fields: &mut BankMusicNodeFields,
) -> Result<u32, F::Error> {
    reader.read_u8()?;
    let count = reader.read_u32()?;
    let value = count.value;
    fields.stinger_count = Some(count);
    Ok(value)
}
