// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{Read, Seek};
use std::ops::Range;

use crate::bank_graph_point_rows::BankGraphPointRows;
use crate::bank_table_rows::BankTableRows;
use crate::hirc_music_track_automation::{self, BankTrackAutomationFields};
use crate::read_at::SeekSource;
use crate::{BankError, BankReadError};

/// Sequential replay of checked `MusicTrack` automation occurrences.
pub struct BankMusicTrackAutomationRows<'input, R>(BankTableRows<'input, R>);

/// One original `MusicTrack` automation with its owner-bound point cursor.
#[non_exhaustive]
pub struct BankMusicTrackAutomation<'input, R> {
    /// Zero-based position in the original automation list.
    pub ordinal: u32,
    /// Successfully read original automation fields.
    pub fields: BankTrackAutomationFields,
    /// Checked original row bytes including a partial final field.
    pub checked_range: Range<u64>,
    /// Format error when the original automation is incomplete.
    pub issue: Option<BankError>,
    source: &'input mut SeekSource<R>,
    stopped: &'input mut bool,
}

impl<'input, R> BankMusicTrackAutomationRows<'input, R> {
    pub(crate) const fn new(
        source: &'input mut SeekSource<R>,
        stopped: &'input mut bool,
        range: &Range<u64>,
        count: u32,
    ) -> Self {
        Self(BankTableRows::new(source, stopped, range, count))
    }
}

impl<R: Read + Seek> BankMusicTrackAutomationRows<'_, R> {
    /// Reads the next original automation whose clip index fits.
    ///
    /// # Errors
    ///
    /// Returns an error if checked source bytes cannot be replayed.
    pub fn next_automation(
        &mut self,
    ) -> Result<Option<BankMusicTrackAutomation<'_, R>>, BankReadError> {
        let Some(row) = self
            .0
            .read(|input, index| hirc_music_track_automation::read_row(input, index))?
        else {
            return Ok(None);
        };
        Ok(Some(BankMusicTrackAutomation {
            ordinal: row.ordinal,
            fields: row.fields,
            checked_range: row.checked_range,
            issue: row.issue,
            source: self.0.source,
            stopped: self.0.stopped,
        }))
    }
}

impl<R> BankMusicTrackAutomation<'_, R> {
    /// Borrows the checked original automation points in source order.
    pub fn points(&mut self) -> Option<BankGraphPointRows<'_, R>> {
        let points = self.fields.points.as_ref()?;
        Some(BankGraphPointRows::new(
            self.source,
            self.stopped,
            &points.extent.checked_range,
            points.declared_count.value,
        ))
    }
}
