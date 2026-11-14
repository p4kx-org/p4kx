// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::hirc_node_state::{self, BankStateBinding, BankStateGroup};
use crate::hirc_state;
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError, SectionId, StateProperties};

/// One source-ordered observation of an inline State chunk.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankInlineStateRow {
    /// Original count of property bindings.
    BindingCount(BankHircField<u32>),
    /// One original binding.
    Binding {
        /// Position among original bindings.
        ordinal: u32,
        /// Original binding fields.
        fields: BankStateBinding,
    },
    /// Original count of State groups.
    GroupCount(BankHircField<u32>),
    /// One original group.
    Group {
        /// Position among original groups.
        ordinal: u32,
        /// Original group header fields.
        fields: BankStateGroup,
    },
    /// One original State in a group.
    State {
        /// Position of the original owning group.
        group: u32,
        /// Position within the original owning group.
        ordinal: u32,
        /// The authored State ID.
        state_id: BankHircField<u32>,
        /// The authored property bundle.
        properties: StateProperties,
    },
}

/// Source-ordered rows within one checked inline State extent.
pub struct BankInlineStateRows<'input, R> {
    fields: SeekObjectFields<'input, R>,
    range: Range<u64>,
    phase: Phase,
    bindings: Range<u32>,
    groups: Range<u32>,
    group: u32,
    states: Range<u32>,
}

#[derive(Clone, Copy)]
enum Phase {
    BindingCount,
    Bindings,
    GroupCount,
    Groups,
    States,
    Done,
}

impl<'input, R> BankInlineStateRows<'input, R> {
    pub(crate) const fn new(input: &'input mut SeekSource<R>, range: Range<u64>) -> Self {
        Self {
            fields: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
            range,
            phase: Phase::BindingCount,
            bindings: 0..0,
            groups: 0..0,
            group: 0,
            states: 0..0,
        }
    }

    /// Returns the checked half-open bank-relative State extent.
    #[must_use]
    pub const fn range(&self) -> &Range<u64> {
        &self.range
    }
}

impl<R: Read + Seek> Iterator for BankInlineStateRows<'_, R> {
    type Item = Result<BankInlineStateRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.next_row() {
            Ok(row) => row.map(Ok),
            Err(cause) => {
                self.phase = Phase::Done;
                Some(Err(cause))
            }
        }
    }
}

impl<R: Read + Seek> BankInlineStateRows<'_, R> {
    fn next_row(&mut self) -> Result<Option<BankInlineStateRow>, BankReadError> {
        loop {
            match self.phase {
                Phase::BindingCount => {
                    let count = hirc_node_state::read_count(&mut self.fields)?;
                    self.bindings = 0..count.value;
                    self.phase = Phase::Bindings;
                    return Ok(Some(BankInlineStateRow::BindingCount(count)));
                }
                Phase::Bindings => {
                    if let Some(ordinal) = self.bindings.next() {
                        let fields = hirc_node_state::read_binding(&mut self.fields)?;
                        return Ok(Some(BankInlineStateRow::Binding { ordinal, fields }));
                    }
                    self.phase = Phase::GroupCount;
                }
                Phase::GroupCount => {
                    let count = hirc_node_state::read_count(&mut self.fields)?;
                    self.groups = 0..count.value;
                    self.phase = Phase::Groups;
                    return Ok(Some(BankInlineStateRow::GroupCount(count)));
                }
                Phase::Groups => {
                    if let Some(ordinal) = self.groups.next() {
                        let fields = hirc_node_state::read_group(&mut self.fields)?;
                        self.group = ordinal;
                        self.states = 0..fields.state_count.value;
                        self.phase = Phase::States;
                        return Ok(Some(BankInlineStateRow::Group { ordinal, fields }));
                    }
                    self.phase = Phase::Done;
                    if self.fields.remaining_bytes() != 0 {
                        return Err(BankError::invalid(
                            SectionId::HIRC,
                            "the checked inline State extent contains unused bytes",
                        )
                        .into());
                    }
                }
                Phase::States => {
                    if let Some(ordinal) = self.states.next() {
                        let state_id = self.fields.read_u32()?;
                        let properties = hirc_state::read_properties(&mut self.fields, 0)?;
                        return Ok(Some(BankInlineStateRow::State {
                            group: self.group,
                            ordinal,
                            state_id,
                            properties,
                        }));
                    }
                    self.phase = Phase::Groups;
                }
                Phase::Done => return Ok(None),
            }
        }
    }
}
