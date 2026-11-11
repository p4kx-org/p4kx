// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields, SeekObjectFields};
use crate::hirc_rtpc::{self, BankRtpcCurve, BankRtpcExtent, BankRtpcPoint};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError, SectionId};

/// A source-ordered curve or point in the checked initial RTPC prefix.
#[derive(Debug)]
#[non_exhaustive]
pub enum BankRtpcRow {
    /// Original curve header.
    Curve {
        /// Position among original curves.
        ordinal: u16,
        /// Original curve header fields.
        fields: BankRtpcCurve,
    },
    /// Original graph point.
    Point {
        /// Position of the owning curve.
        curve: u16,
        /// Position within the owning curve.
        ordinal: u16,
        /// Original graph point fields.
        fields: BankRtpcPoint,
    },
}

/// Source-ordered rows from the count and complete curve prefix.
pub struct BankRtpcRows<'input, R> {
    fields: SeekObjectFields<'input, R>,
    extent: BankRtpcExtent,
    curves: Range<u16>,
    points: Range<u16>,
    curve: u16,
    finished: bool,
}

impl<'input, R> BankRtpcRows<'input, R> {
    pub(crate) fn new(input: &'input mut SeekSource<R>, extent: &BankRtpcExtent) -> Self {
        let start = extent.curve_count.range.end;
        let end = extent.checked_range.end;
        Self {
            fields: SeekObjectFields::after_id(input, start, end - start, 0),
            extent: extent.clone(),
            curves: 0..extent.checked_curves,
            points: 0..0,
            curve: 0,
            finished: false,
        }
    }

    /// Returns the original count and its half-open bank-relative range.
    #[must_use]
    pub const fn declared_curve_count(&self) -> &BankHircField<u16> {
        &self.extent.curve_count
    }

    /// Returns the number of complete curves in the original checked prefix.
    #[must_use]
    pub const fn checked_curve_count(&self) -> u16 {
        self.extent.checked_curves
    }

    /// Returns the bank-relative extent of the count and complete curve prefix.
    #[must_use]
    pub const fn range(&self) -> &Range<u64> {
        &self.extent.checked_range
    }
}

impl<R: Read + Seek> Iterator for BankRtpcRows<'_, R> {
    type Item = Result<BankRtpcRow, BankReadError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.finished {
            return None;
        }
        match self.next_row() {
            Ok(Some(row)) => Some(Ok(row)),
            Ok(None) => {
                self.finished = true;
                None
            }
            Err(error) => {
                self.finished = true;
                Some(Err(error))
            }
        }
    }
}

impl<R: Read + Seek> BankRtpcRows<'_, R> {
    fn next_row(&mut self) -> Result<Option<BankRtpcRow>, BankReadError> {
        if let Some(ordinal) = self.points.next() {
            let fields = hirc_rtpc::read_point(&mut self.fields)?;
            return Ok(Some(BankRtpcRow::Point {
                curve: self.curve,
                ordinal,
                fields,
            }));
        }
        if let Some(ordinal) = self.curves.next() {
            let fields = hirc_rtpc::read_curve(&mut self.fields)?;
            self.curve = ordinal;
            self.points = 0..fields.point_count.value;
            return Ok(Some(BankRtpcRow::Curve { ordinal, fields }));
        }
        if self.fields.remaining_bytes() != 0 {
            return Err(BankError::invalid(
                SectionId::HIRC,
                "the checked RTPC extent contains unused bytes",
            )
            .into());
        }
        Ok(None)
    }
}
