// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, Record};

/// The physical DataForge Record selected by a palette lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PaletteRecordSelection {
    /// The Record index in the DataForge supplied to the provider.
    pub index: u32,
    /// The GUID of the selected Record.
    pub guid: Guid,
}

impl PaletteRecordSelection {
    pub(super) const fn of(record: &Record<'_>) -> Self {
        Self {
            index: record.index(),
            guid: record.guid(),
        }
    }
}

/// The selected value of a palette lookup and its physical Record coordinate.
#[derive(Debug)]
pub struct PaletteRead<T> {
    /// The value supplied by the palette provider.
    pub value: T,
    /// The Record that supplied the value when the provider can prove it.
    pub record: Option<PaletteRecordSelection>,
}

impl<T> PaletteRead<T> {
    pub(super) const fn unobserved(value: T) -> Self {
        Self {
            value,
            record: None,
        }
    }
}

/// The Records used to select and read one palette.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PaletteRecords {
    /// The Record selected by a default-palette path lookup.
    pub selector: Option<PaletteRecordSelection>,
    /// The Record whose payload supplied the palette.
    pub palette: Option<PaletteRecordSelection>,
}
