// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::dos_calendar::DosCalendar;

/// The two unmodified DOS words stored in an Entry's directory record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DosTimestamp {
    date: u16,
    time: u16,
}

impl DosTimestamp {
    pub(crate) const fn new(date: u16, time: u16) -> Self {
        Self { date, time }
    }

    /// Returns the complete stored date word, including invalid or zero encodings.
    #[must_use]
    pub const fn raw_date(&self) -> u16 {
        self.date
    }

    /// Returns the complete stored time word, including invalid or zero encodings.
    #[must_use]
    pub const fn raw_time(&self) -> u16 {
        self.time
    }

    /// Returns valid calendar fields without a timezone, or none for invalid encodings.
    #[must_use]
    pub const fn calendar(&self) -> Option<DosCalendar> {
        DosCalendar::new(self.date, self.time)
    }
}
