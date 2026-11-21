// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use super::{HircKind, HircValue};
use crate::BankError;
use crate::bank_object_fields::BankHircField;
use crate::bank_reader::SectionReader;
use crate::bank_structure::{OBJECT_HEADER_LEN, OBJECT_ID_LEN, counted_id_byte_range};
use crate::hirc_action::{self, BankActionFields};
use crate::hirc_event;

/// One object of the `HIRC` section, with its ID and its decoded value.
#[derive(Debug, Clone, PartialEq)]
pub struct HircObject {
    /// The type of the object.
    pub kind: HircKind,
    /// The object ID.
    pub id: u32,
    /// What this reader decodes from the body.
    pub value: HircValue,
    pub(crate) body: Range<usize>,
    pub(super) route: HircRouteEvidence,
}

impl HircObject {
    /// Returns the whole object span in the decoded bank, including its envelope and ID.
    #[must_use]
    pub const fn byte_range(&self) -> Range<usize> {
        self.body.start - OBJECT_HEADER_LEN..self.body.end
    }

    /// Returns the original object type byte in the decoded bank.
    #[must_use]
    pub const fn kind_byte_range(&self) -> Range<usize> {
        let start = self.body.start - OBJECT_HEADER_LEN;
        start..start + 1
    }

    /// Returns the four bytes of the object ID in the decoded bank.
    #[must_use]
    pub const fn id_byte_range(&self) -> Range<usize> {
        self.body.start..self.body.start + OBJECT_ID_LEN
    }

    /// Returns the original count of a decoded Event's actions and its bank-relative range.
    #[must_use]
    pub const fn event_action_count_evidence(&self) -> Option<&BankHircField<u32>> {
        match &self.route {
            HircRouteEvidence::Event(count) => Some(count),
            _ => None,
        }
    }

    /// Returns the bank-relative range of a decoded Event's action ID at its physical ordinal.
    #[must_use]
    pub fn event_action_id_byte_range(&self, ordinal: usize) -> Option<Range<u64>> {
        counted_id_byte_range(self.event_action_count_evidence()?, ordinal)
    }

    /// Returns the original reads and bank-relative ranges of a decoded Action.
    #[must_use]
    pub fn action_evidence(&self) -> Option<&BankActionFields> {
        match &self.route {
            HircRouteEvidence::Action(fields) => Some(fields),
            _ => None,
        }
    }

    /// Returns why the body could not be decoded, for a [`HircValue::Malformed`] object.
    #[must_use]
    pub const fn error(&self) -> Option<&BankError> {
        match &self.value {
            HircValue::Malformed(error) => Some(error),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) enum HircRouteEvidence {
    #[default]
    None,
    Event(BankHircField<u32>),
    Action(Box<BankActionFields>),
}

impl HircRouteEvidence {
    pub(super) fn read_event(
        &mut self,
        version: u32,
        fields: &mut SectionReader<'_>,
        bank_offset: u64,
    ) -> Result<HircValue, BankError> {
        let (actions, count) = hirc_event::read(version, fields, bank_offset)?;
        *self = Self::Event(count);
        Ok(HircValue::Event(actions))
    }

    pub(super) fn read_action(
        &mut self,
        version: u32,
        fields: &mut SectionReader<'_>,
        bank_offset: u64,
    ) -> Result<HircValue, BankError> {
        let (target, observed) = hirc_action::action(version, fields, bank_offset)?;
        *self = Self::Action(Box::new(observed));
        Ok(HircValue::Action(target))
    }
}
