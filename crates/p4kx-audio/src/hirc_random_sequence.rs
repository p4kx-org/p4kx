// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

pub(crate) const WORD_LEN: usize = size_of::<u32>();
pub(crate) const ITEM_LEN: usize = 2 * WORD_LEN;

/// Original decision fields following the shared base of a version-154 container.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankRandomSequenceFields {
    /// Start of the original decision continuation.
    pub start: u64,
    /// Original loop count, including zero.
    pub loop_count: Option<BankHircField<u16>>,
    /// Original minimum loop modifier.
    pub loop_minimum: Option<BankHircField<u16>>,
    /// Original maximum loop modifier.
    pub loop_maximum: Option<BankHircField<u16>>,
    /// Original binary32 transition time word.
    pub transition_time: Option<BankHircField<u32>>,
    /// Original binary32 minimum transition modifier.
    pub transition_minimum: Option<BankHircField<u32>>,
    /// Original binary32 maximum transition modifier.
    pub transition_maximum: Option<BankHircField<u32>>,
    /// Original avoid-repeat count.
    pub avoid_repeat_count: Option<BankHircField<u16>>,
    /// Original transition mode code.
    pub transition_mode: Option<BankHircField<u8>>,
    /// Original random selection mode code.
    pub random_mode: Option<BankHircField<u8>>,
    /// Original flags, including unknown bits.
    pub flags: Option<BankHircField<u8>>,
    /// Original playlist count and checked extent once reached.
    pub playlist: Option<BankRandomSequencePlaylist>,
}

impl BankRandomSequenceFields {
    pub(crate) const fn new(start: u64) -> Self {
        Self {
            start,
            loop_count: None,
            loop_minimum: None,
            loop_maximum: None,
            transition_time: None,
            transition_minimum: None,
            transition_maximum: None,
            avoid_repeat_count: None,
            transition_mode: None,
            random_mode: None,
            flags: None,
            playlist: None,
        }
    }

    /// Returns the decision extent once the original playlist fits.
    #[must_use]
    pub fn complete_range(&self) -> Option<Range<u64>> {
        let playlist = self.playlist.as_ref()?;
        playlist
            .is_complete()
            .then_some(self.start..playlist.checked_range.end)
    }
}

/// The original playlist count and checked prefix of its physical items.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankRandomSequencePlaylist {
    /// Original authored count, including zero.
    pub declared_count: BankHircField<u16>,
    /// Checked original item bytes, including a partial final item.
    pub checked_range: Range<u64>,
}

impl BankRandomSequencePlaylist {
    /// Returns whether all declared playlist items fit.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.checked_range.end - self.checked_range.start
            == u64::from(self.declared_count.value) * ITEM_LEN as u64
    }
}

pub(crate) fn before_mode<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankRandomSequenceFields,
) -> Result<(), F::Error> {
    observed.loop_count = Some(input.read_u16()?);
    observed.loop_minimum = Some(input.read_u16()?);
    observed.loop_maximum = Some(input.read_u16()?);
    observed.transition_time = Some(input.read_u32()?);
    observed.transition_minimum = Some(input.read_u32()?);
    observed.transition_maximum = Some(input.read_u32()?);
    observed.avoid_repeat_count = Some(input.read_u16()?);
    observed.transition_mode = Some(input.read_u8()?);
    observed.random_mode = Some(input.read_u8()?);
    Ok(())
}

pub(crate) fn playlist<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankRandomSequenceFields,
) -> Result<(), F::Error> {
    let count = input.read_u16()?;
    let start = input.offset();
    let required = u64::from(count.value) * ITEM_LEN as u64;
    observed.playlist = Some(BankRandomSequencePlaylist {
        declared_count: count,
        checked_range: start..start + required.min(input.remaining_bytes()),
    });
    input.skip_bytes(required)
}
