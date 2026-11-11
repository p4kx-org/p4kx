// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::bank_object_fields::{BankHircField, ObjectFields};
use crate::hirc_rtpc::CURVE_POINT_LEN;

/// Checked bytes of a version-154 `MusicTrack` automation table.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankTrackAutomationExtent {
    /// Original table bytes, including any incomplete final field.
    pub checked_range: Range<u64>,
    complete: bool,
}

impl BankTrackAutomationExtent {
    /// Returns whether all declared rows fit the original body.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.complete
    }
}

/// Original `MusicTrack` automation point count and checked byte extent.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankTrackAutomationPoints {
    /// Original authored point count.
    pub declared_count: BankHircField<u32>,
    /// Checked original point table.
    pub extent: BankTrackAutomationExtent,
}

/// Original authored fields of one `MusicTrack` automation occurrence.
#[derive(Debug)]
#[non_exhaustive]
pub struct BankTrackAutomationFields {
    /// Original clip index.
    pub clip_index: BankHircField<u32>,
    /// Original automation type, including unknown codes.
    pub automation_type: Option<BankHircField<u32>>,
    /// Original point count and checked graph words when reached.
    pub points: Option<BankTrackAutomationPoints>,
}

pub(crate) fn scan<F: ObjectFields>(
    input: &mut F,
    count: u32,
) -> (BankTrackAutomationExtent, Result<(), F::Error>) {
    let start = input.offset();
    let read = scan_rows(input, count);
    let end = if read.is_ok() {
        input.offset()
    } else {
        input.offset() + input.remaining_bytes()
    };
    (
        BankTrackAutomationExtent {
            checked_range: start..end,
            complete: read.is_ok(),
        },
        read,
    )
}

fn scan_rows<F: ObjectFields>(input: &mut F, count: u32) -> Result<(), F::Error> {
    for _ in 0..count {
        let index = input.read_u32()?;
        read_row(input, index).1?;
    }
    Ok(())
}

pub(crate) fn read_row<F: ObjectFields>(
    input: &mut F,
    clip_index: BankHircField<u32>,
) -> (BankTrackAutomationFields, Result<(), F::Error>) {
    let mut observed = BankTrackAutomationFields {
        clip_index,
        automation_type: None,
        points: None,
    };
    let result = points(input, &mut observed);
    (observed, result)
}

fn points<F: ObjectFields>(
    input: &mut F,
    observed: &mut BankTrackAutomationFields,
) -> Result<(), F::Error> {
    observed.automation_type = Some(input.read_u32()?);
    let declared_count = input.read_u32()?;
    let start = input.offset();
    let required = u64::from(declared_count.value) * CURVE_POINT_LEN as u64;
    let available = input.remaining_bytes();
    observed.points = Some(BankTrackAutomationPoints {
        declared_count,
        extent: BankTrackAutomationExtent {
            checked_range: start..start + required.min(available),
            complete: required <= available,
        },
    });
    input.skip_bytes(required)
}
