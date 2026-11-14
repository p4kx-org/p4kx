// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};

pub(crate) const SLOT_LEN: usize = 1 + size_of::<u32>() + 1;

/// An authored effect or metadata list in a version-154 node base.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankNodeFxKind {
    /// Initial node effects, with a conditional bypass-all byte.
    Effects,
    /// Initial node metadata effects.
    Metadata,
}

/// Checked prefix of an authored effect or metadata list.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankNodeFxFields {
    /// Original bank-relative list position when reached by the shared reader.
    pub start: Option<u64>,
    /// Raw parent-override byte.
    pub override_parent_bits: Option<BankHircField<u8>>,
    /// Raw bypass-all byte, authored only for a nonempty effects list.
    pub bypass_all_bits: Option<BankHircField<u8>>,
    /// Checked bank-relative slot bytes, including any incomplete final slot.
    pub slots_range: Option<Range<u64>>,
}

/// Original scalar fields and slot extent of one authored list.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct BankNodeFxList<'source> {
    /// Original one-byte slot count when readable.
    pub declared_count: Option<&'source BankHircField<u8>>,
    /// Independently readable original fields and extent.
    pub fields: &'source BankNodeFxFields,
}

impl BankNodeFxList<'_> {
    /// Returns the complete original list extent when all authored fields fit.
    #[must_use]
    pub fn complete_range(self) -> Option<Range<u64>> {
        let start = self.fields.override_parent_bits.as_ref()?.range.start;
        let count = self.declared_count?.value;
        let slots = self.fields.slots_range.as_ref()?;
        let expected = u64::from(count) * SLOT_LEN as u64;
        (slots.end - slots.start == expected).then_some(start..slots.end)
    }
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    kind: BankNodeFxKind,
    observed: &mut BankNodeFxFields,
    declared: &mut Option<BankHircField<u8>>,
) -> Result<(), F::Error> {
    observed.start = Some(input.offset());
    observed.override_parent_bits = Some(input.read_u8()?);
    let count = input.read_u8()?;
    let required = u64::from(count.value) * SLOT_LEN as u64;
    *declared = Some(count);
    if required > 0 && kind == BankNodeFxKind::Effects {
        observed.bypass_all_bits = Some(input.read_u8()?);
    }
    let start = input.offset();
    observed.slots_range = Some(start..start + input.remaining_bytes().min(required));
    input.skip_bytes(required)
}
