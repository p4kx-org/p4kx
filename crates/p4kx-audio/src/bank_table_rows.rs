// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError};

const ID_LEN: u64 = size_of::<u32>() as u64;

pub(crate) struct BankTableRows<'input, R> {
    pub(crate) source: &'input mut SeekSource<R>,
    pub(crate) stopped: &'input mut bool,
    range: Range<u64>,
    ordinals: Range<u32>,
}

pub(crate) struct BankTableRow<T> {
    pub(crate) ordinal: u32,
    pub(crate) fields: T,
    pub(crate) checked_range: Range<u64>,
    pub(crate) issue: Option<BankError>,
}

impl<'input, R> BankTableRows<'input, R> {
    pub(crate) const fn new(
        source: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        range: &Range<u64>,
        count: u32,
    ) -> Self {
        Self {
            source,
            stopped,
            range: range.start..range.end,
            ordinals: 0..count,
        }
    }
}

impl<R: Read + Seek> BankTableRows<'_, R> {
    pub(crate) fn read<T>(
        &mut self,
        scan: impl FnOnce(
            &mut SeekObjectFields<'_, R>,
            BankHircField<u32>,
        ) -> (T, Result<(), BankReadError>),
    ) -> Result<Option<BankTableRow<T>>, BankReadError> {
        let start = self.range.start;
        let available = self.range.end - start;
        if *self.stopped || available < ID_LEN {
            return Ok(None);
        }
        let Some(ordinal) = self.ordinals.next() else {
            return Ok(None);
        };
        let mut input = SeekObjectFields::after_id(self.source, start, available, 0);
        let id = input.read_u32().inspect_err(|_| *self.stopped = true)?;
        let (fields, result) = scan(&mut input, id);
        let (end, issue) = match result {
            Ok(()) => (input.offset(), None),
            Err(BankReadError::Format(error)) => (self.range.end, Some(error)),
            Err(error) => {
                *self.stopped = true;
                return Err(error);
            }
        };
        self.range.start = end;
        Ok(Some(BankTableRow {
            ordinal,
            fields,
            checked_range: start..end,
            issue,
        }))
    }
}
