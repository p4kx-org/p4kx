// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::SectionId;
use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_node_state;
use crate::hirc_plugin_parameters::{self, BankPluginFields};
use crate::hirc_rtpc::{self, BankRtpcExtent};

/// The original effect-base fields in a version-154 plug-in owner.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct BankEffectFields {
    /// Original plug-in ID, opaque parameters and referenced bank data.
    pub plugin: BankPluginFields,
    /// Checked initial RTPC section when its original count fits.
    pub rtpc: Option<BankRtpcExtent>,
    /// Exact complete inline State range when the full mapping fits.
    pub states: Option<Range<u64>>,
    /// Authored number of plug-in state property overrides.
    pub override_count: Option<BankHircField<u16>>,
    /// Original complete override prefix, ordered by the physical declaration.
    pub override_range: Option<Range<u64>>,
}

impl BankEffectFields {
    pub(crate) fn checked_end(&self) -> Option<u64> {
        self.override_range
            .as_ref()
            .map(|range| range.end)
            .or_else(|| self.states.as_ref().map(|range| range.end))
            .or_else(|| self.rtpc.as_ref().map(|extent| extent.checked_range.end))
            .or_else(|| self.plugin.checked_end())
    }
}

/// A state property override bound to its original effect owner.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct BankEffectOverride {
    /// Original variable-length property ID.
    pub property_id: BankHircField<u32>,
    /// Original accumulation code.
    pub accumulation: BankHircField<u8>,
    /// Original value word, with its literal bits.
    pub value_bits: BankHircField<u32>,
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    fields: &mut BankEffectFields,
) -> Result<(), F::Error> {
    hirc_plugin_parameters::scan(input, &mut fields.plugin)?;
    hirc_rtpc::scan(input, &mut fields.rtpc)?;
    fields.states = Some(hirc_node_state::skip(input)?);
    let count = input.read_u16()?;
    let declared = count.value;
    fields.override_count = Some(count);
    let start = input.offset();
    let checked = fields.override_range.insert(start..start);
    for _ in 0..declared {
        read_override(input)?;
        checked.end = input.offset();
    }
    Ok(())
}

pub(crate) fn read_override<F: ObjectFields>(
    input: &mut F,
) -> Result<BankEffectOverride, F::Error> {
    Ok(BankEffectOverride {
        property_id: input.read_variable_length(SectionId::HIRC)?,
        accumulation: input.read_u8()?,
        value_bits: input.read_u32()?,
    })
}
