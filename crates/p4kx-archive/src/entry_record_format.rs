// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The on-disk directory record that supplies an Entry's metadata.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum EntryRecordFormat {
    /// A ZIP central-directory record, including the P4K v1 layout.
    ZipCentralDirectory,
    /// A fixed P4K v2 directory record.
    P4kV2,
}
