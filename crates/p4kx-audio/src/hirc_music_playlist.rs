// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music::MusicPlaylistItem;

pub(crate) const PLAYLIST_ITEM_LEN: usize = 30;

/// One original playlist item and the exact ranges of its words.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicPlaylistItem {
    /// Original segment ID, zero for a group.
    pub segment_id: BankHircField<u32>,
    /// Original item ID.
    pub item_id: BankHircField<u32>,
    /// Original number of following child items.
    pub children: BankHircField<u32>,
    /// Original play mode code.
    pub play_mode: BankHircField<u32>,
    /// Original signed loop count.
    pub loops: BankHircField<i16>,
    /// Original signed lower loop bound.
    pub loop_min: BankHircField<i16>,
    /// Original signed upper loop bound.
    pub loop_max: BankHircField<i16>,
    /// Original sibling weight.
    pub weight: BankHircField<u32>,
    /// Original avoid-repeat count.
    pub avoid_repeat_count: BankHircField<u16>,
    /// Original weight selector byte.
    pub uses_weight_code: BankHircField<u8>,
    /// Original shuffle selector byte.
    pub shuffle_code: BankHircField<u8>,
    /// Complete checked original item range.
    pub range: Range<u64>,
}

impl BankMusicPlaylistItem {
    /// Returns the playlist item with its legacy boolean flag interpretations.
    #[must_use]
    pub const fn value(&self) -> MusicPlaylistItem {
        MusicPlaylistItem {
            segment_id: self.segment_id.value,
            item_id: self.item_id.value,
            children: self.children.value,
            play_mode: self.play_mode.value,
            loops: self.loops.value,
            loop_min: self.loop_min.value,
            loop_max: self.loop_max.value,
            weight: self.weight.value,
            avoid_repeat_count: self.avoid_repeat_count.value,
            uses_weight: self.uses_weight_code.value != 0,
            shuffle: self.shuffle_code.value != 0,
        }
    }
}

pub(crate) fn item<F: ObjectFields>(fields: &mut F) -> Result<BankMusicPlaylistItem, F::Error> {
    let start = fields.offset();
    Ok(BankMusicPlaylistItem {
        segment_id: fields.read_u32()?,
        item_id: fields.read_u32()?,
        children: fields.read_u32()?,
        play_mode: fields.read_u32()?,
        loops: fields.scalar(i16::from_le_bytes)?,
        loop_min: fields.scalar(i16::from_le_bytes)?,
        loop_max: fields.scalar(i16::from_le_bytes)?,
        weight: fields.read_u32()?,
        avoid_repeat_count: fields.read_u16()?,
        uses_weight_code: fields.read_u8()?,
        shuffle_code: fields.read_u8()?,
        range: start..fields.offset(),
    })
}
