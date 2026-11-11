// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::tree_cursor::Cursor;
use crate::tree_owner::TreeOwner;
use crate::{Field, Guid, Location, Record, RecordMetadata, StructValue, Value, ValueError};

/// The canonical tree of one Record: its GUID, record path, name and root struct with the fields.
///
/// It is the single model every output is made from; text is borrowed from the DataForge.
#[derive(Debug, Clone)]
pub struct RecordTree<'f> {
    pub(crate) record_index: u32,
    pub(crate) metadata: RecordMetadata<'f>,
    pub(crate) guid: Guid,
    pub(crate) record_path: &'f str,
    pub(crate) name: &'f str,
    pub(crate) tag: Option<&'f str>,
    pub(crate) root: StructValue<'f>,
    pub(crate) owner: TreeOwner<'f>,
}

impl<'f> RecordTree<'f> {
    /// Returns whether this tree was built from this exact original Record owner and position.
    ///
    /// Equal values, GUIDs or indices do not establish original ownership.
    #[must_use]
    pub fn is_from(&self, record: &Record<'_>) -> bool {
        self.record_index == record.index() && self.owner.is_from(record)
    }

    /// Returns original borrowed metadata independently of normalized legacy header getters.
    #[must_use]
    pub const fn metadata(&self) -> RecordMetadata<'f> {
        self.metadata
    }

    /// Returns the owning Record position in source definition order.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// Returns the name of the Record's struct, e.g. `EntityClassDefinition`.
    #[must_use]
    pub const fn type_name(&self) -> &'f str {
        self.root.type_name
    }

    /// Returns the GUID that identifies the Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path, without `.xml`.
    #[must_use]
    pub const fn record_path(&self) -> &'f str {
        self.record_path
    }

    /// Returns the Record name, e.g. `EntityClassDefinition.AEGS_Avenger`.
    #[must_use]
    pub const fn name(&self) -> &'f str {
        self.name
    }

    /// Returns the Record tag, e.g. `SystemsDesign`; `None` for a version 6 DataForge and for a
    /// Record with an empty or no tag.
    #[must_use]
    pub const fn tag(&self) -> Option<&'f str> {
        self.tag
    }

    /// Returns the Record's own struct, to walk it like any nested struct.
    #[must_use]
    pub const fn root(&self) -> &StructValue<'f> {
        &self.root
    }

    /// Returns the fields in declaration order, the inherited ones first.
    #[must_use]
    pub fn fields(&self) -> &[Field<'f>] {
        &self.root.fields
    }

    /// Returns the value of the first field called `name`, or `None` when there is none.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Value<'f>> {
        self.root.field(name)
    }

    /// Returns each error with its actual containing field name in declaration order.
    pub fn errors(&self) -> impl Iterator<Item = (&'f str, &ValueError)> {
        let mut cursor = Cursor::new(&self.root.fields);
        std::iter::from_fn(move || {
            loop {
                let visit = cursor.next()?;
                if let Value::Error(error) = visit.value {
                    return Some((visit.field, error));
                }
            }
        })
    }

    /// Returns each error with its complete semantic location in declaration order.
    pub fn located_errors(&self) -> impl Iterator<Item = (Location<'f>, &ValueError)> {
        let mut cursor = Cursor::new(&self.root.fields);
        std::iter::from_fn(move || {
            loop {
                let visit = cursor.next()?;
                if let Value::Error(error) = visit.value {
                    return Some((Location::new(visit.steps), error));
                }
            }
        })
    }

    /// Counts every error occurrence without collecting locations.
    #[must_use]
    pub fn error_count(&self) -> usize {
        let mut cursor = Cursor::new(&self.root.fields);
        let mut count = 0;
        while let Some(visit) = cursor.next() {
            count += usize::from(matches!(visit.value, Value::Error(_)));
        }
        count
    }
}

impl PartialEq for RecordTree<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.record_index == other.record_index
            && self.metadata == other.metadata
            && self.guid == other.guid
            && self.record_path == other.record_path
            && self.name == other.name
            && self.tag == other.tag
            && self.root == other.root
    }
}
