// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::BankReadError;
use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::hirc_rtpc::{self, CURVE_POINT_LEN};
use crate::read_at::SeekSource;

const WORD_LEN: u64 = size_of::<u32>() as u64;

/// One original graph point with its checked word prefix.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankGraphPoint {
    /// Zero-based ordinal in the authored graph.
    pub ordinal: u32,
    /// Original binary32 input word.
    pub from_bits: BankHircField<u32>,
    /// Original binary32 output word when readable.
    pub to_bits: Option<BankHircField<u32>>,
    /// Original interpolation word, including unknown codes.
    pub interpolation: Option<BankHircField<u32>>,
    /// Checked bytes of the original graph point.
    pub checked_range: Range<u64>,
}

/// Fixed-memory cursor over checked graph points.
pub struct BankGraphPointRows<'input, R> {
    input: SeekObjectFields<'input, R>,
    ordinals: Range<u32>,
    stopped: &'input mut bool,
}

impl<'input, R> BankGraphPointRows<'input, R> {
    pub(crate) const fn new(
        input: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        range: &Range<u64>,
        count: u32,
    ) -> Self {
        Self {
            input: SeekObjectFields::after_id(input, range.start, range.end - range.start, 0),
            ordinals: 0..count,
            stopped,
        }
    }
}

impl<R: Read + Seek> Iterator for BankGraphPointRows<'_, R> {
    type Item = Result<BankGraphPoint, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if *self.stopped || self.input.remaining_bytes() < WORD_LEN {
            return None;
        }
        let ordinal = self.ordinals.next()?;
        let read = self.read(ordinal);
        if read.is_err() {
            *self.stopped = true;
        }
        Some(read)
    }
}

impl<R: Read + Seek> BankGraphPointRows<'_, R> {
    fn read(&mut self, ordinal: u32) -> Result<BankGraphPoint, BankReadError> {
        let start = self.input.offset();
        let available = self.input.remaining_bytes().min(CURVE_POINT_LEN as u64);
        let (from_bits, to_bits, interpolation) = if available == CURVE_POINT_LEN as u64 {
            let point = hirc_rtpc::read_point(&mut self.input)?;
            (
                point.from_bits,
                Some(point.to_bits),
                Some(point.interpolation),
            )
        } else {
            let from_bits = self.input.read_u32()?;
            let to_bits = (self.input.remaining_bytes() >= WORD_LEN)
                .then(|| self.input.read_u32())
                .transpose()?;
            (from_bits, to_bits, None)
        };
        Ok(BankGraphPoint {
            ordinal,
            from_bits,
            to_bits,
            interpolation,
            checked_range: start..start + available,
        })
    }
}
