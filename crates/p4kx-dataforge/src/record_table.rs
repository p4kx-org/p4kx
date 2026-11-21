// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ByteReader;

use crate::DataForgeError;
use crate::rows::RecordDef;
use crate::table::{Fields, take};

/// The row layout of the Record definition table, which is the one table that differs between
/// the supported versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum RecordLayout {
    /// Name, file name, struct index, GUID, instance index and size: 32 bytes.
    V6,
    /// The version 6 row with a tag after the file name: 36 bytes.
    V8,
}

impl RecordLayout {
    /// Returns the layout of `version`, or `None` for a version this reader does not know.
    pub(crate) const fn for_version(version: u32) -> Option<Self> {
        match version {
            6 => Some(Self::V6),
            8 => Some(Self::V8),
            _ => None,
        }
    }

    const fn row_len(self) -> usize {
        match self {
            Self::V6 => 32,
            Self::V8 => 36,
        }
    }

    const fn decode(self, row: &[u8]) -> RecordDef {
        let mut fields = Fields::new(row);
        let name = fields.u32();
        let file_name = fields.u32();
        let tag = match self {
            Self::V6 => None,
            Self::V8 => Some(fields.u32()),
        };
        let struct_index = fields.u32();
        let guid = fields.bytes16();
        let instance_index = fields.u16();
        RecordDef {
            name,
            file_name,
            tag,
            struct_index,
            guid,
            instance_index,
            trailing: fields.u16(),
        }
    }
}

/// A view of the Record definition table in the layout of its version.
#[derive(Debug, Clone, Copy)]
pub(crate) struct RecordTable<'a> {
    bytes: &'a [u8],
    layout: RecordLayout,
}

impl<'a> RecordTable<'a> {
    /// Takes a table of `count` rows in `layout` from `reader`.
    pub(crate) fn read(
        reader: &mut ByteReader<'a>,
        count: u32,
        layout: RecordLayout,
    ) -> Result<Self, DataForgeError> {
        let bytes = take(reader, count, layout.row_len(), "Record table")?;
        Ok(Self { bytes, layout })
    }

    pub(crate) const fn bytes(&self) -> &'a [u8] {
        self.bytes
    }

    pub(crate) const fn len(&self) -> usize {
        self.bytes.len() / self.layout.row_len()
    }

    pub(crate) fn get(&self, index: usize) -> Option<RecordDef> {
        let len = self.layout.row_len();
        let start = index.checked_mul(len)?;
        let row = self.bytes.get(start..start.checked_add(len)?)?;
        Some(self.layout.decode(row))
    }

    pub(crate) fn iter(&self) -> impl ExactSizeIterator<Item = RecordDef> + use<'a> {
        let layout = self.layout;
        self.bytes
            .chunks_exact(layout.row_len())
            .map(move |row| layout.decode(row))
    }

    pub(crate) fn iter_indexed(&self) -> impl ExactSizeIterator<Item = (u32, RecordDef)> + use<'a> {
        let len = u32::try_from(self.len()).unwrap_or(u32::MAX);
        (0..len).zip(self.iter())
    }
}
