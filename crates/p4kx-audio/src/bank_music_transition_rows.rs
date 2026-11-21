// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::bank_table_rows::BankTableRows;
use crate::hirc_music_transition::BankMusicTransitionFields;
use crate::hirc_music_transition_rule::{
    self, BankMusicTransitionIds, BankMusicTransitionRuleFields,
};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError};

const WORD_LEN: u64 = size_of::<u32>() as u64;

/// One of the two authored ID lists in a music transition rule.
#[derive(Clone, Copy)]
pub enum BankMusicTransitionSide {
    /// Source IDs.
    Source,
    /// Destination IDs.
    Destination,
}

/// Sequential replay of checked original transition rules.
pub struct BankMusicTransitionRows<'input, R>(BankTableRows<'input, R>);

/// One original transition rule and its owner-bound ID cursors.
#[non_exhaustive]
pub struct BankMusicTransitionRule<'input, R> {
    /// Zero-based ordinal in the original rule list.
    pub ordinal: u32,
    /// Original fields of the physical rule.
    pub fields: BankMusicTransitionRuleFields,
    /// Checked original rule bytes including a partial final field.
    pub checked_range: Range<u64>,
    /// Format error when the original rule is incomplete.
    pub issue: Option<BankError>,
    source: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
}

impl<'input, R> BankMusicTransitionRows<'input, R> {
    pub(crate) fn new(
        source: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        fields: &BankMusicTransitionFields,
    ) -> Option<Self> {
        let range = fields.checked_range.as_ref()?;
        let count = fields.rule_count.as_ref()?.value;
        Some(Self(BankTableRows::new(source, stopped, range, count)))
    }
}

impl<R: Read + Seek> BankMusicTransitionRows<'_, R> {
    /// Reads the next original rule whose source count fits.
    ///
    /// # Errors
    ///
    /// Returns an error if checked source bytes cannot be replayed.
    pub fn next_rule(&mut self) -> Result<Option<BankMusicTransitionRule<'_, R>>, BankReadError> {
        let Some(row) = self
            .0
            .read(|input, count| hirc_music_transition_rule::read(input, count))?
        else {
            return Ok(None);
        };
        Ok(Some(BankMusicTransitionRule {
            ordinal: row.ordinal,
            fields: row.fields,
            checked_range: row.checked_range,
            issue: row.issue,
            source: self.0.source,
            stopped: self.0.stopped,
        }))
    }
}

impl<R> BankMusicTransitionRule<'_, R> {
    /// Borrows checked original IDs from the selected list.
    pub fn ids(
        &mut self,
        side: BankMusicTransitionSide,
    ) -> Option<BankMusicTransitionIdRows<'_, R>> {
        let list = match side {
            BankMusicTransitionSide::Source => &self.fields.source_ids,
            BankMusicTransitionSide::Destination => self.fields.destination_ids.as_ref()?,
        };
        Some(BankMusicTransitionIdRows::new(
            self.source,
            self.stopped,
            list,
        ))
    }
}

/// One physical original transition ID occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankMusicTransitionId {
    /// Zero-based ordinal within the original ID list.
    pub ordinal: u32,
    /// Original reference word, including any sentinel value.
    pub id: BankHircField<u32>,
}

/// Fixed-memory replay of the checked original ID prefix.
pub struct BankMusicTransitionIdRows<'input, R> {
    input: SeekObjectFields<'input, R>,
    ordinals: Range<u32>,
    stopped: &'input mut bool,
}

impl<'input, R> BankMusicTransitionIdRows<'input, R> {
    const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        list: &BankMusicTransitionIds,
    ) -> Self {
        let range = &list.checked_range;
        Self {
            input: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
            ordinals: 0..list.declared_count.value,
            stopped,
        }
    }
}

impl<R: Read + Seek> Iterator for BankMusicTransitionIdRows<'_, R> {
    type Item = Result<BankMusicTransitionId, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped || self.input.remaining_bytes() < WORD_LEN {
            return None;
        }
        let ordinal = self.ordinals.next()?;
        let id = self.input.read_u32().inspect_err(|_| *self.stopped = true);
        Some(id.map(|id| BankMusicTransitionId { ordinal, id }))
    }
}
