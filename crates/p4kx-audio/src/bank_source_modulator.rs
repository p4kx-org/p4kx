// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_source_effect_metadata;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_modulator::{self, BankModulatorFields, BankModulatorKind};
use crate::hirc_node_property::BankNodePropertyKind;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError};

/// Owner-bound original modulation properties, ranges and RTPC graphs.
pub struct BankHircModulator<'input, R> {
    input: &'input mut SeekSource<R>,
    object: BankHircObject,
    kind: BankModulatorKind,
    fields: BankModulatorFields,
    error: Option<BankError>,
}

impl<R> BankHircModulator<'_, R> {
    /// Returns the original physical HIRC occurrence.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns the authored modulation kind.
    #[must_use]
    pub const fn kind(&self) -> BankModulatorKind {
        self.kind
    }

    /// Returns the checked original field groups.
    #[must_use]
    pub const fn fields(&self) -> &BankModulatorFields {
        &self.fields
    }

    /// Returns the format error for an incomplete or incorrectly bounded body.
    #[must_use]
    pub const fn format_error(&self) -> Option<&BankError> {
        self.error.as_ref()
    }

    /// Returns the original checked extent including the object ID.
    #[must_use]
    pub fn checked_range(&self) -> Range<u64> {
        let start = self.object.body_offset();
        let end = self
            .fields
            .checked_end()
            .unwrap_or(start + OBJECT_ID_LEN as u64);
        start..end
    }

    /// Returns the unprojected original body suffix.
    #[must_use]
    pub fn unprojected_range(&self) -> Option<Range<u64>> {
        bank_source_effect_metadata::checked_suffix(&self.object, self.checked_range().end)
    }
}

impl<R: Read + Seek> BankHircModulator<'_, R> {
    /// Replays checked original property or ranged modifier rows.
    pub fn properties(
        &mut self,
        kind: BankNodePropertyKind,
    ) -> Option<BankNodePropertyRows<'_, R>> {
        let extent = self.fields.properties.get(kind)?;
        Some(BankNodePropertyRows::new(self.input, extent))
    }

    /// Replays checked original RTPC graph rows.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        let extent = self.fields.rtpc.as_ref()?;
        Some(BankRtpcRows::new(self.input, extent))
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &mut bool,
    object: BankHircObject,
    kind: BankModulatorKind,
) -> Result<BankHircModulator<'input, R>, BankReadError> {
    let mut fields = BankModulatorFields::default();
    let error = bank_source_effect_metadata::observe(input, stopped, &object, |reader| {
        hirc_modulator::scan(reader, &mut fields)
    })?;
    Ok(BankHircModulator {
        input,
        object,
        kind,
        fields,
        error,
    })
}
