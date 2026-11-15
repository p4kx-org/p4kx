// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::HircKind;
use crate::bank_object_fields::ObjectFields;
use crate::hirc_node_property::{self, BankNodePropertyExtents, BankNodePropertyKind};
use crate::hirc_rtpc::{self, BankRtpcExtent};

/// The physical kind of a version-154 modulator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum BankModulatorKind {
    /// Low-frequency oscillator.
    Lfo,
    /// Envelope.
    Envelope,
    /// Time modulator.
    Time,
}

impl BankModulatorKind {
    /// Returns a modulator kind when the physical HIRC type is recognized.
    #[must_use]
    pub const fn of(kind: HircKind) -> Option<Self> {
        match kind {
            HircKind::Lfo => Some(Self::Lfo),
            HircKind::Envelope => Some(Self::Envelope),
            HircKind::TimeModulator => Some(Self::Time),
            _ => None,
        }
    }
}

/// Original property, range and RTPC lists shared by the three v154 modulator kinds.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankModulatorFields {
    /// Independent checked property and ranged modifier lists.
    pub properties: BankNodePropertyExtents,
    /// Original RTPC count and checked prefix of graph rows.
    pub rtpc: Option<BankRtpcExtent>,
}

impl BankModulatorFields {
    pub(crate) fn checked_end(&self) -> Option<u64> {
        self.rtpc
            .as_ref()
            .map(|extent| extent.checked_range.end)
            .or_else(|| property_end(&self.properties, BankNodePropertyKind::RangedModifiers))
            .or_else(|| property_end(&self.properties, BankNodePropertyKind::Properties))
    }
}

fn property_end(lists: &BankNodePropertyExtents, kind: BankNodePropertyKind) -> Option<u64> {
    let extent = lists.get(kind)?;
    Some(extent.words_range.as_ref().unwrap_or(&extent.ids_range).end)
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankModulatorFields,
) -> Result<(), F::Error> {
    hirc_node_property::scan(input, &mut fields.properties)?;
    hirc_rtpc::scan(input, &mut fields.rtpc)?;
    input.require_exhausted("bytes follow the modulator object")
}
