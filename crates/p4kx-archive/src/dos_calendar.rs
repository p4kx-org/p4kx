// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

const YEAR_BASE: u16 = 1980;
const YEAR_SHIFT: u16 = 9;
const MONTH_SHIFT: u16 = 5;
const HOUR_SHIFT: u16 = 11;
const MINUTE_SHIFT: u16 = 5;
const MONTH_MASK: u16 = 0x0f;
const FIVE_BITS: u16 = 0x1f;
const MINUTE_MASK: u16 = 0x3f;

/// Validated DOS calendar fields without a timezone or UTC interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DosCalendar {
    year: u16,
    month: u8,
    day: u8,
    hour: u8,
    minute: u8,
    second: u8,
}

impl DosCalendar {
    pub(crate) const fn new(date: u16, time: u16) -> Option<Self> {
        let year = YEAR_BASE + (date >> YEAR_SHIFT);
        let month = ((date >> MONTH_SHIFT) & MONTH_MASK) as u8;
        let day = (date & FIVE_BITS) as u8;
        let hour = (time >> HOUR_SHIFT) as u8;
        let minute = ((time >> MINUTE_SHIFT) & MINUTE_MASK) as u8;
        let second = (time & FIVE_BITS) as u8 * 2;
        let last_day = match month {
            1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
            4 | 6 | 9 | 11 => 30,
            2 if year.is_multiple_of(400)
                || year.is_multiple_of(4) && !year.is_multiple_of(100) =>
            {
                29
            }
            2 => 28,
            _ => return None,
        };
        if day == 0 || day > last_day || hour > 23 || minute > 59 || second > 59 {
            return None;
        }
        Some(Self {
            year,
            month,
            day,
            hour,
            minute,
            second,
        })
    }

    /// Returns the year in the DOS range 1980 through 2107.
    #[must_use]
    pub const fn year(&self) -> u16 {
        self.year
    }

    /// Returns the month from 1 through 12.
    #[must_use]
    pub const fn month(&self) -> u8 {
        self.month
    }

    /// Returns the valid day of the stated month.
    #[must_use]
    pub const fn day(&self) -> u8 {
        self.day
    }

    /// Returns the hour from 0 through 23.
    #[must_use]
    pub const fn hour(&self) -> u8 {
        self.hour
    }

    /// Returns the minute from 0 through 59.
    #[must_use]
    pub const fn minute(&self) -> u8 {
        self.minute
    }

    /// Returns the even second from 0 through 58.
    #[must_use]
    pub const fn second(&self) -> u8 {
        self.second
    }
}
