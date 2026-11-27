// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_music_fade::{self, BankMusicFadeFields};

const WORD_LEN: u64 = size_of::<u32>() as u64;

/// Original reference count and checked bytes of one music transition ID list.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicTransitionIds {
    /// Original authored count.
    pub declared_count: BankHircField<u32>,
    /// Checked ID bytes, including an incomplete final word.
    pub checked_range: Range<u64>,
}

impl BankMusicTransitionIds {
    fn new<F: ObjectFields>(input: &F, declared_count: BankHircField<u32>) -> Self {
        let start = input.offset();
        let bytes = u64::from(declared_count.value) * WORD_LEN;
        Self {
            declared_count,
            checked_range: start..start + bytes.min(input.remaining_bytes()),
        }
    }

    /// Returns whether all declared IDs fit their original body.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.checked_range.end - self.checked_range.start == self.required_bytes()
    }

    fn required_bytes(&self) -> u64 {
        u64::from(self.declared_count.value) * WORD_LEN
    }
}

/// Original version-154 fields of one physical music transition rule.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicTransitionRuleFields {
    /// Original source ID table.
    pub source_ids: BankMusicTransitionIds,
    /// Original destination ID table when reached.
    pub destination_ids: Option<BankMusicTransitionIds>,
    /// Original source rule words.
    pub source: BankMusicTransitionSourceFields,
    /// Original destination rule words.
    pub destination: BankMusicTransitionDestinationFields,
    /// Original transition object allocation byte when readable.
    pub allocation_flag: Option<BankHircField<u8>>,
    /// Original transition object fields when allocated and reached.
    pub transition_object: Option<BankMusicTransitionObjectFields>,
}

/// Original source-side scheduling words of a music transition.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTransitionSourceFields {
    /// Original source fade.
    pub fade: BankMusicFadeFields,
    /// Original synchronization code.
    pub sync_type: Option<BankHircField<u32>>,
    /// Original cue filter hash.
    pub cue_filter_hash: Option<BankHircField<u32>>,
    /// Original play-post-exit byte.
    pub play_post_exit: Option<BankHircField<u8>>,
}

/// Original destination-side scheduling words of a music transition.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTransitionDestinationFields {
    /// Original destination fade.
    pub fade: BankMusicFadeFields,
    /// Original cue filter hash.
    pub cue_filter_hash: Option<BankHircField<u32>>,
    /// Original jump target ID.
    pub jump_to_id: Option<BankHircField<u32>>,
    /// Original jump-selection code.
    pub jump_to_type: Option<BankHircField<u16>>,
    /// Original entry type code.
    pub entry_type: Option<BankHircField<u16>>,
    /// Original play-pre-entry byte.
    pub play_pre_entry: Option<BankHircField<u8>>,
    /// Original destination match-source-cue byte.
    pub match_source_cue_name: Option<BankHircField<u8>>,
}

/// Original scheduling words of an allocated music transition object.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankMusicTransitionObjectFields {
    /// Original segment reference.
    pub segment_id: Option<BankHircField<u32>>,
    /// Original fade-in fields.
    pub fade_in: BankMusicFadeFields,
    /// Original fade-out fields.
    pub fade_out: BankMusicFadeFields,
    /// Original play-pre-entry byte.
    pub play_pre_entry: Option<BankHircField<u8>>,
    /// Original play-post-exit byte.
    pub play_post_exit: Option<BankHircField<u8>>,
}

pub(crate) fn read<F: ObjectFields>(
    input: &mut F,
    count: BankHircField<u32>,
) -> (BankMusicTransitionRuleFields, Result<(), F::Error>) {
    let mut observed = BankMusicTransitionRuleFields {
        source_ids: BankMusicTransitionIds::new(input, count),
        destination_ids: None,
        source: BankMusicTransitionSourceFields::default(),
        destination: BankMusicTransitionDestinationFields::default(),
        allocation_flag: None,
        transition_object: None,
    };
    let result = remaining_rule(input, &mut observed);
    (observed, result)
}

fn remaining_rule<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTransitionRuleFields,
) -> Result<(), F::Error> {
    input.skip_bytes(observed.source_ids.required_bytes())?;
    let count = input.read_u32()?;
    let destination = observed
        .destination_ids
        .insert(BankMusicTransitionIds::new(input, count));
    input.skip_bytes(destination.required_bytes())?;
    source(input, &mut observed.source)?;
    destination_rule(input, &mut observed.destination)?;
    let allocation = input.read_u8()?;
    let allocated = allocation.value != 0;
    observed.allocation_flag = Some(allocation);
    if allocated {
        let object = observed
            .transition_object
            .insert(BankMusicTransitionObjectFields::default());
        transition_object(input, object)?;
    }
    Ok(())
}

fn source<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTransitionSourceFields,
) -> Result<(), F::Error> {
    hirc_music_fade::read(input, &mut observed.fade)?;
    observed.sync_type = Some(input.read_u32()?);
    observed.cue_filter_hash = Some(input.read_u32()?);
    observed.play_post_exit = Some(input.read_u8()?);
    Ok(())
}

fn destination_rule<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTransitionDestinationFields,
) -> Result<(), F::Error> {
    hirc_music_fade::read(input, &mut observed.fade)?;
    observed.cue_filter_hash = Some(input.read_u32()?);
    observed.jump_to_id = Some(input.read_u32()?);
    observed.jump_to_type = Some(input.read_u16()?);
    observed.entry_type = Some(input.read_u16()?);
    observed.play_pre_entry = Some(input.read_u8()?);
    observed.match_source_cue_name = Some(input.read_u8()?);
    Ok(())
}

fn transition_object<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankMusicTransitionObjectFields,
) -> Result<(), F::Error> {
    observed.segment_id = Some(input.read_u32()?);
    hirc_music_fade::read(input, &mut observed.fade_in)?;
    hirc_music_fade::read(input, &mut observed.fade_out)?;
    observed.play_pre_entry = Some(input.read_u8()?);
    observed.play_post_exit = Some(input.read_u8()?);
    Ok(())
}
