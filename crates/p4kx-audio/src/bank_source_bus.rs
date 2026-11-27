// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_inline_state_rows::BankInlineStateRows;
use crate::bank_node_fx_slots::BankNodeFxSlots;
use crate::bank_node_property_rows::BankNodePropertyRows;
use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_positioning_rows::BankPositioningRows;
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_node_aux::{self, BankNodeUserAuxBuses};
use crate::hirc_node_fx::BankNodeFxKind;
use crate::hirc_node_positioning::BankPositioningTableKind;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError};

#[path = "bank_bus_ducks.rs"]
mod ducks;

pub use self::ducks::{BankBusDuckRow, BankBusDuckRows};
use crate::hirc_bus_fields as fields;
pub use crate::hirc_bus_fields::{BankHircBusFields, BankHircBusSettings};

/// Version-154 bus fields and replay cursors bound to their owning bank.
pub struct BankHircBus<'input, R> {
    input: &'input mut SeekSource<R>,
    object: BankHircObject,
    fields: BankHircBusFields,
    error: Option<BankError>,
    parsed_end: u64,
}

impl<R> BankHircBus<'_, R> {
    /// Returns the original physical HIRC occurrence.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns independently readable fields and checked array extents.
    #[must_use]
    pub const fn fields(&self) -> &BankHircBusFields {
        &self.fields
    }

    /// Returns the format error for a malformed or trailing region.
    #[must_use]
    pub const fn format_error(&self) -> Option<&BankError> {
        self.error.as_ref()
    }

    /// Returns the bank-relative suffix past the sequential scan.
    #[must_use]
    pub const fn raw_remainder(&self) -> Range<u64> {
        self.parsed_end..self.object.body_end_exclusive()
    }

    /// Borrows the original planar property list if its count was read.
    pub fn properties(&mut self) -> Option<BankNodePropertyRows<'_, R>> {
        Some(BankNodePropertyRows::new(
            self.input,
            self.fields.properties.as_ref()?,
        ))
    }

    /// Borrows the checked prefix of duck targets.
    pub fn ducks(&mut self) -> Option<BankBusDuckRows<'_, R>> {
        Some(BankBusDuckRows::new(
            self.input,
            self.fields.duck_rows.clone()?,
        ))
    }

    /// Borrows checked values in the selected positioning table.
    pub fn positioning_rows(
        &mut self,
        kind: BankPositioningTableKind,
    ) -> Option<BankPositioningRows<'_, R>> {
        let range = self.fields.positioning.table(kind)?.clone();
        Some(BankPositioningRows::new(self.input, kind, range))
    }

    /// Borrows checked effect or metadata slot prefixes.
    pub fn fx_slots(&mut self, kind: BankNodeFxKind) -> Option<BankNodeFxSlots<'_, R>> {
        let range = self.fields.fx(kind).fields.slots_range.clone()?;
        Some(BankNodeFxSlots::new(self.input, range))
    }

    /// Borrows the readable RTPC header and its complete curves.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        Some(BankRtpcRows::new(
            self.input,
            self.fields.rtpc_chunk.as_ref()?,
        ))
    }

    /// Borrows inline State rows when the whole chunk was checked.
    pub fn states(&mut self) -> Option<BankInlineStateRows<'_, R>> {
        Some(BankInlineStateRows::new(
            self.input,
            self.fields.state_chunk.clone()?,
        ))
    }
}

impl<R: Read + Seek> BankHircBus<'_, R> {
    /// Replays the checked user Aux send IDs by authored slot.
    ///
    /// # Errors
    /// Returns the error if the checked source bytes cannot be read.
    pub fn user_aux_buses(&mut self) -> Result<Option<BankNodeUserAuxBuses>, BankReadError> {
        hirc_node_aux::user_buses(self.input, &self.fields.aux)
    }
}

pub(super) fn observe<R: Read + Seek>(
    input: &mut SeekSource<R>,
    object: BankHircObject,
) -> Result<BankHircBus<'_, R>, BankReadError> {
    let mut fields = BankHircBusFields::default();
    let mut cursor = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    let error = match fields::scan(&mut cursor, &mut fields) {
        Ok(()) => None,
        Err(BankReadError::Format(error)) => Some(error),
        Err(error) => return Err(error),
    };
    let parsed_end = cursor.offset();
    Ok(BankHircBus {
        input,
        object,
        fields,
        error,
        parsed_end,
    })
}
