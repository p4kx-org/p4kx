// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt;
use std::sync::Arc;

use crate::reference_origin::ReferenceOrigin;
use crate::tree_owner::TreeOwner;
use crate::{Field, Guid, Location, Value};

/// A struct inside a Record, or the Record's own fields: an inline class, the target of a strong
/// pointer, or the root of a [`RecordTree`](crate::RecordTree).
#[derive(Debug, Clone, PartialEq)]
pub struct StructValue<'f> {
    pub(crate) type_name: &'f str,
    pub(crate) fields: Vec<Field<'f>>,
}

impl<'f> StructValue<'f> {
    /// Returns the name of the struct, which for a strong pointer may derive from the declared
    /// one.
    #[must_use]
    pub const fn type_name(&self) -> &'f str {
        self.type_name
    }

    /// Returns the fields in declaration order, the inherited ones first.
    #[must_use]
    pub fn fields(&self) -> &[Field<'f>] {
        &self.fields
    }

    /// Returns the value of the first field called `name`, or `None` when there is none.
    #[must_use]
    pub fn field(&self, name: &str) -> Option<&Value<'f>> {
        self.fields
            .iter()
            .find(|field| field.name == name)
            .map(Field::value)
    }
}

/// A link to a Record by GUID, never its content (ADR-0004).
///
/// A Reference field links to another Record. A weak pointer links into the Record that holds
/// it: to the same GUID and record path, with the [`location`](Self::location) of the instance.
#[derive(Clone)]
pub struct Reference<'f> {
    pub(crate) owner: TreeOwner<'f>,
    pub(crate) record_index: u32,
    pub(crate) guid: Guid,
    pub(crate) record_path: &'f str,
    pub(crate) type_name: &'f str,
    pub(crate) location: Option<Arc<Location<'f>>>,
}

impl PartialEq for Reference<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.record_index == other.record_index
            && self.guid == other.guid
            && self.record_path == other.record_path
            && self.type_name == other.type_name
            && self.location == other.location
    }
}

impl Eq for Reference<'_> {}

impl fmt::Debug for Reference<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Reference")
            .field("record_index", &self.record_index)
            .field("guid", &self.guid)
            .field("record_path", &self.record_path)
            .field("type_name", &self.type_name)
            .field("location", &self.location)
            .finish()
    }
}

impl<'f> Reference<'f> {
    /// Borrows the original paired target admission without looking it up again.
    #[must_use]
    pub const fn original_target(&self) -> ReferenceOrigin<'_, 'f> {
        ReferenceOrigin::new(self)
    }

    /// Returns the actual target Record position in source definition order.
    #[must_use]
    pub const fn record_index(&self) -> u32 {
        self.record_index
    }

    /// Returns the GUID of the target Record.
    #[must_use]
    pub const fn guid(&self) -> Guid {
        self.guid
    }

    /// Returns the record path of the target Record, without `.xml`.
    #[must_use]
    pub const fn record_path(&self) -> &'f str {
        self.record_path
    }

    /// Returns the type of the target: the struct of the Record, or of the instance at the
    /// location.
    #[must_use]
    pub const fn type_name(&self) -> &'f str {
        self.type_name
    }

    /// Returns where inside the target Record the link points, or `None` for the Record itself.
    #[must_use]
    pub fn location(&self) -> Option<&Location<'f>> {
        self.location.as_deref()
    }
}
