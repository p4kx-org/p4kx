// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::SeekObjectFields;
use crate::bank_structure::OBJECT_ID_LEN;
use crate::read_at::SeekSource;
use crate::{BankError, BankHircObject, BankReadError};

pub(crate) fn observe<R: Read + Seek>(
    input: &mut SeekSource<R>,
    stopped: &mut bool,
    object: &BankHircObject,
    parse: impl FnOnce(&mut SeekObjectFields<'_, R>) -> Result<(), BankReadError>,
) -> Result<Option<BankError>, BankReadError> {
    let mut reader = SeekObjectFields::after_id(
        input,
        object.body_offset(),
        object.body_len(),
        OBJECT_ID_LEN as u64,
    );
    match parse(&mut reader) {
        Ok(()) => Ok(None),
        Err(BankReadError::Format(cause)) => Ok(Some(cause)),
        Err(cause) => {
            *stopped = true;
            Err(cause)
        }
    }
}

pub(crate) const fn checked_suffix(
    object: &BankHircObject,
    checked_end: u64,
) -> Option<Range<u64>> {
    let end = object.body_end_exclusive();
    if checked_end < end {
        Some(checked_end..end)
    } else {
        None
    }
}
