// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use schemars::JsonSchema;
use serde::Serialize;

use crate::{ChangedValue, DataForgeError, Guid, Location};

/// How a Record differs between the two sides of a Comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum RecordChangeKind {
    /// Only the new side has the GUID.
    Added,
    /// Only the old side has the GUID.
    Removed,
    /// Both sides have the GUID at different record paths; fields may differ as well.
    Moved,
    /// Both sides have the GUID at the same record path, with a different name, tag, type or field.
    Changed,
    /// The Record cannot be read on at least one side, so it was not compared.
    Unreadable,
}

/// One side of a Comparison.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RecordSide {
    /// The old Archive.
    Old,
    /// The new Archive.
    New,
}

/// Why a Record cannot be read on one side.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordError {
    pub(crate) side: RecordSide,
    pub(crate) error: DataForgeError,
}

impl RecordError {
    /// Returns the side whose Record cannot be read.
    #[must_use]
    pub const fn side(&self) -> RecordSide {
        self.side
    }

    /// Returns what cannot be read.
    #[must_use]
    pub const fn error(&self) -> &DataForgeError {
        &self.error
    }
}

/// What identifies a Record on one side: its record path, name, tag and type.
#[derive(Debug, Clone, Copy)]
pub struct RecordState<'a> {
    pub(crate) record_path: &'a str,
    pub(crate) name: &'a str,
    pub(crate) tag: Option<&'a str>,
    pub(crate) type_name: &'a str,
    pub(crate) structure: u32,
}

impl PartialEq for RecordState<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.record_path == other.record_path
            && self.name == other.name
            && self.tag == other.tag
            && self.type_name == other.type_name
    }
}

impl Eq for RecordState<'_> {}

impl<'a> RecordState<'a> {
    /// Returns the record path, without `.xml`.
    #[must_use]
    pub const fn record_path(&self) -> &'a str {
        self.record_path
    }

    /// Returns the Record name, e.g. `EntityClassDefinition.AEGS_Avenger`.
    #[must_use]
    pub const fn name(&self) -> &'a str {
        self.name
    }

    /// Returns the Record tag, e.g. `SystemsDesign`; `None` for a version 6 DataForge and for a
    /// Record with an empty or no tag.
    #[must_use]
    pub const fn tag(&self) -> Option<&'a str> {
        self.tag
    }

    /// Returns the name of the Record's struct.
    #[must_use]
    pub const fn type_name(&self) -> &'a str {
        self.type_name
    }
}

/// One field or array element whose value differs, or that only one side has.
#[derive(Debug, Clone, PartialEq)]
pub struct FieldChange<'a> {
    pub(crate) path: Location<'a>,
    pub(crate) old: Option<ChangedValue<'a>>,
    pub(crate) new: Option<ChangedValue<'a>>,
}

impl<'a> FieldChange<'a> {
    /// Returns where the value lies in the Record, e.g. `hull.plates[1].armor`.
    #[must_use]
    pub const fn path(&self) -> &Location<'a> {
        &self.path
    }

    /// Returns the old value, `None` when the old side has no such field or element.
    #[must_use]
    pub const fn old_value(&self) -> Option<&ChangedValue<'a>> {
        self.old.as_ref()
    }

    /// Returns the new value, `None` when the new side has no such field or element.
    #[must_use]
    pub const fn new_value(&self) -> Option<&ChangedValue<'a>> {
        self.new.as_ref()
    }
}

/// Where the Records of one Record change lie in both DataForges: their positions in
/// definition order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RecordKey {
    old: Option<u32>,
    new: Option<u32>,
}

impl RecordKey {
    /// Returns the key of the Records at `old` in the old and at `new` in the new DataForge.
    #[must_use]
    pub(crate) const fn new(old: Option<u32>, new: Option<u32>) -> Self {
        Self { old, new }
    }

    /// Returns the position in the old DataForge, `None` for an added Record.
    #[must_use]
    pub const fn old_position(self) -> Option<u32> {
        self.old
    }

    /// Returns the position in the new DataForge, `None` for a removed Record.
    #[must_use]
    pub const fn new_position(self) -> Option<u32> {
        self.new
    }
}

/// One Record that was added, removed, moved or changed.
#[derive(Debug, Clone, PartialEq)]
pub struct RecordChange<'a> {
    pub(crate) key: RecordKey,
    pub(crate) guid: Guid,
    pub(crate) old: Option<RecordState<'a>>,
    pub(crate) new: Option<RecordState<'a>>,
    pub(crate) fields: Vec<FieldChange<'a>>,
    pub(crate) errors: Vec<RecordError>,
}

impl<'a> RecordChange<'a> {
    /// Returns how the Record differs.
    #[must_use]
    pub fn kind(&self) -> RecordChangeKind {
        if !self.errors.is_empty() {
            return RecordChangeKind::Unreadable;
        }
        match (self.old, self.new) {
            (Some(old), Some(new))
                if !crate::path_key::eq_normalised(old.record_path, new.record_path) =>
            {
                RecordChangeKind::Moved
            }
            (Some(_), Some(_)) => RecordChangeKind::Changed,
            (Some(_), None) => RecordChangeKind::Removed,
            (None, _) => RecordChangeKind::Added,
        }
    }

    /// Returns the GUID that joins both sides.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns where both Records lie, for comparing this one Record again.
    #[must_use]
    pub const fn key(&self) -> RecordKey {
        self.key
    }

    /// Returns the record path of the new side, or of the old side when the new side has none;
    /// empty when neither record path can be read.
    #[must_use]
    pub fn path(&self) -> &'a str {
        self.new.or(self.old).map_or("", |side| side.record_path)
    }

    /// Returns the Record on the old side, `None` when it was added or cannot be read.
    #[must_use]
    pub const fn old_record(&self) -> Option<RecordState<'a>> {
        self.old
    }

    /// Returns the Record on the new side, `None` when it was removed or cannot be read.
    #[must_use]
    pub const fn new_record(&self) -> Option<RecordState<'a>> {
        self.new
    }

    /// Returns the changed fields and array elements, nested ones included, depth first in field
    /// order of the new side with the fields only the old side has last in each struct; empty
    /// for an added or removed Record.
    #[must_use]
    pub fn fields(&self) -> &[FieldChange<'a>] {
        &self.fields
    }

    /// Returns why the Record cannot be read, one entry per side; empty unless it is unreadable.
    #[must_use]
    pub fn errors(&self) -> &[RecordError] {
        &self.errors
    }
}

/// The number of Records per change kind.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, JsonSchema)]
#[non_exhaustive]
pub struct RecordCounts {
    /// Records only the new side has.
    pub added: usize,
    /// Records only the old side has.
    pub removed: usize,
    /// Records at another record path.
    pub moved: usize,
    /// Records at the same record path with a different name, type or field.
    pub changed: usize,
    /// Records that cannot be read on at least one side.
    pub unreadable: usize,
}

impl RecordCounts {
    /// Counts `changes` per change kind.
    pub(crate) fn of(changes: &[RecordChange<'_>]) -> Self {
        let mut counts = Self::default();
        for change in changes {
            let count = match change.kind() {
                RecordChangeKind::Added => &mut counts.added,
                RecordChangeKind::Removed => &mut counts.removed,
                RecordChangeKind::Moved => &mut counts.moved,
                RecordChangeKind::Changed => &mut counts.changed,
                RecordChangeKind::Unreadable => &mut counts.unreadable,
            };
            *count += 1;
        }
        counts
    }

    /// Returns the number of Records of every change kind.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.added + self.removed + self.moved + self.changed + self.unreadable
    }
}
