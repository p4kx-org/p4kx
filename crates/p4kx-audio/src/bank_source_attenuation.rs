// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{ObjectFields, SeekObjectFields};
use crate::bank_rtpc_rows::BankRtpcRows;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::hirc_attenuation::{self, BankAttenuationFields};
use crate::hirc_rtpc::{self, BankRtpcPoint};
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError};

/// Original attenuation fields and bounded point replay for a physical HIRC object.
pub struct BankHircAttenuation<'input, R> {
    input: &'input mut SeekSource<R>,
    object: BankHircObject,
    fields: BankAttenuationFields,
    error: Option<BankError>,
}

impl<R> BankHircAttenuation<'_, R> {
    /// Returns the original object envelope and identity.
    #[must_use]
    pub const fn object(&self) -> &BankHircObject {
        &self.object
    }

    /// Returns the original fields observed before the parser completed or stopped.
    #[must_use]
    pub const fn fields(&self) -> &BankAttenuationFields {
        &self.fields
    }

    /// Returns the error for an incomplete or incorrectly bounded proved layout.
    #[must_use]
    pub const fn format_error(&self) -> Option<&BankError> {
        self.error.as_ref()
    }

    /// Returns the checked original byte range including the object ID.
    #[must_use]
    pub const fn checked_range(&self) -> Range<u64> {
        self.object.body_offset()..self.fields.checked_end
    }

    /// Returns the original unprojected suffix, including partial words and trailing bytes.
    #[must_use]
    pub fn unprojected_range(&self) -> Option<Range<u64>> {
        let end = self.object.body_end_exclusive();
        (self.fields.checked_end < end).then_some(self.fields.checked_end..end)
    }

    /// Replays original complete points of one physical conversion curve.
    pub fn points(&mut self, curve: usize) -> Option<BankAttenuationPoints<'_, R>> {
        let range = &self.fields.curves.get(curve)?.points_range;
        Some(BankAttenuationPoints {
            fields: SeekObjectFields::after_id(self.input, range.start, range.end - range.start, 0),
            failed: false,
        })
    }

    /// Replays original initial RTPC curves and points.
    pub fn rtpc(&mut self) -> Option<BankRtpcRows<'_, R>> {
        self.fields
            .rtpc
            .as_ref()
            .map(|extent| BankRtpcRows::new(self.input, extent))
    }
}

/// Bounded replay of the complete original points of an attenuation curve.
pub struct BankAttenuationPoints<'input, R> {
    fields: SeekObjectFields<'input, R>,
    failed: bool,
}

impl<R: Read + Seek> Iterator for BankAttenuationPoints<'_, R> {
    type Item = Result<BankRtpcPoint, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.failed || self.fields.remaining_bytes() == 0 {
            return None;
        }
        let point = hirc_rtpc::read_point(&mut self.fields);
        self.failed = point.is_err();
        Some(point)
    }
}

pub(crate) fn observe<'input, R: Read + Seek>(
    input: &'input mut SeekSource<R>,
    stopped: &mut bool,
    object: BankHircObject,
) -> Result<BankHircAttenuation<'input, R>, BankReadError> {
    let mut reader = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    let mut fields = BankAttenuationFields::default();
    let error = match hirc_attenuation::scan(&mut reader, &mut fields) {
        Ok(()) => None,
        Err(BankReadError::Format(error)) => Some(error),
        Err(error) => {
            *stopped = true;
            return Err(error);
        }
    };
    Ok(BankHircAttenuation {
        input,
        object,
        fields,
        error,
    })
}
