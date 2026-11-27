// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::ops::Range;

use crate::{ObservedResolution, PositionedValue};

/// A column of one published flat value row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum FlatColumn {
    /// GUID of the owning Record.
    RecordGuid,
    /// Name of the owning Record.
    RecordName,
    /// Path of the owning Record.
    RecordPath,
    /// Escaped path of the selected field.
    FieldPath,
    /// Original array ordinals along the field path.
    ArrayIndices,
    /// Output category of the selected value.
    Kind,
    /// The selected scalar or Reference GUID.
    Value,
    /// Path of the selected Reference target.
    TargetPath,
    /// The text selected by the localisation resolver.
    Text,
    /// Tag on the owning Record.
    RecordTag,
}

impl FlatColumn {
    /// Returns the canonical column or NDJSON property name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RecordGuid => "record_guid",
            Self::RecordName => "record_name",
            Self::RecordPath => "record_path",
            Self::FieldPath => "field_path",
            Self::ArrayIndices => "array_indices",
            Self::Kind => "kind",
            Self::Value => "value",
            Self::TargetPath => "target_path",
            Self::Text => "text",
            Self::RecordTag => "record_tag",
        }
    }
}

/// A half-open interval of one actually emitted column value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlatColumnSpan {
    pub(super) column: FlatColumn,
    pub(super) start: u64,
    pub(super) end: u64,
}

impl FlatColumnSpan {
    /// Returns the emitted column.
    #[must_use]
    pub const fn column(&self) -> FlatColumn {
        self.column
    }

    /// Returns its exact formatted value bytes relative to this Record's write.
    #[must_use]
    pub const fn bytes(&self) -> Range<u64> {
        self.start..self.end
    }
}

/// One emitted leaf row and the original positions used to reach its value.
pub struct FlatRowObservation<'row, 'record, 'source, 'text> {
    pub(super) row_index: u64,
    pub(super) field_path: &'row str,
    pub(super) array_indices: &'row [u32],
    pub(super) columns: &'row [FlatColumnSpan],
    pub(super) route: &'row [PositionedValue<'record, 'source>],
    pub(super) field_path_aliases: &'row [PositionedValue<'record, 'source>],
    pub(super) localization: Option<&'row ObservedResolution<'text>>,
}

impl<'row, 'record, 'source, 'text> FlatRowObservation<'row, 'record, 'source, 'text> {
    /// Returns this Record's zero-based emitted row ordinal.
    #[must_use]
    pub const fn row_index(&self) -> u64 {
        self.row_index
    }

    /// Returns the emitted field path before output escaping.
    #[must_use]
    pub const fn field_path(&self) -> &'row str {
        self.field_path
    }

    /// Returns the original array ordinals in traversal order.
    #[must_use]
    pub const fn array_indices(&self) -> &'row [u32] {
        self.array_indices
    }

    /// Returns only columns emitted by the format, in write order.
    #[must_use]
    pub const fn columns(&self) -> &'row [FlatColumnSpan] {
        self.columns
    }

    /// Returns the original enclosing field and array slots, followed by the emitted leaf.
    #[must_use]
    pub const fn route(&self) -> &'row [PositionedValue<'record, 'source>] {
        self.route
    }

    /// Returns original preceding fields whose duplicate names required an ordinal escape.
    #[must_use]
    pub const fn field_path_aliases(&self) -> &'row [PositionedValue<'record, 'source>] {
        self.field_path_aliases
    }

    /// Returns the resolver decision used by this row, including a decision to emit no text.
    #[must_use]
    pub const fn localization(&self) -> Option<&'row ObservedResolution<'text>> {
        self.localization
    }
}

pub(super) type FlatObserver<'observer, 'record, 'source, 'text> =
    dyn for<'row> FnMut(FlatRowObservation<'row, 'record, 'source, 'text>) -> io::Result<()>
        + 'observer;
