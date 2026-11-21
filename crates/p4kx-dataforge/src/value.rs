// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::field_storage::FieldStorage;
use crate::{Guid, Reference, StructValue, ValueError};

/// The value of one field or array element in the canonical Record tree.
///
/// Text is borrowed from the DataForge. Owned children are nested; other Records and weak
/// pointers are [`Value::Reference`] links and never embedded. A value that cannot be read is an
/// [`Value::Error`] at its place, so the rest of the Record still builds.
#[non_exhaustive]
pub enum Value<'f> {
    /// A boolean.
    Bool(bool),
    /// A signed 8-bit integer.
    Int8(i8),
    /// A signed 16-bit integer.
    Int16(i16),
    /// A signed 32-bit integer.
    Int32(i32),
    /// A signed 64-bit integer.
    Int64(i64),
    /// An unsigned 8-bit integer.
    UInt8(u8),
    /// An unsigned 16-bit integer.
    UInt16(u16),
    /// An unsigned 32-bit integer.
    UInt32(u32),
    /// An unsigned 64-bit integer.
    UInt64(u64),
    /// A 32-bit float.
    Single(f32),
    /// A 64-bit float.
    Double(f64),
    /// A string.
    String(&'f str),
    /// A localisation key, kept as the key, e.g. `@item_Name`.
    Locale(&'f str),
    /// A GUID.
    Guid(Guid),
    /// The name of an enum option.
    Enum(&'f str),
    /// An owned struct: an inline class or the target of a strong pointer.
    Struct(StructValue<'f>),
    /// The elements of an array field in stored order.
    Array(Box<[Self]>),
    /// A link to a Record, or to an instance inside the Record for a weak pointer.
    Reference(Box<Reference<'f>>),
    /// A null pointer or a null Reference.
    Null,
    /// A value that cannot be read, with the reason.
    Error(ValueError),
}

impl Value<'_> {
    /// Returns whether the value is a scalar: a bool, number, string, localisation key, GUID or
    /// enum option, but no struct, array, Reference, null or error.
    #[must_use]
    pub const fn is_scalar(&self) -> bool {
        matches!(
            self,
            Self::Bool(_)
                | Self::Int8(_)
                | Self::Int16(_)
                | Self::Int32(_)
                | Self::Int64(_)
                | Self::UInt8(_)
                | Self::UInt16(_)
                | Self::UInt32(_)
                | Self::UInt64(_)
                | Self::Single(_)
                | Self::Double(_)
                | Self::String(_)
                | Self::Locale(_)
                | Self::Guid(_)
                | Self::Enum(_)
        )
    }
}

/// One field of a Record or struct value: its name and its value.
#[derive(Debug, Clone)]
pub struct Field<'f> {
    pub(crate) storage: FieldStorage,
    pub(crate) name: &'f str,
    pub(crate) value: Value<'f>,
}

impl PartialEq for Field<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.name == other.name && self.value == other.value
    }
}

impl<'f> Field<'f> {
    /// Returns the declaration used by the original decoder of this field and its elements.
    #[must_use]
    pub const fn storage(&self) -> FieldStorage {
        self.storage
    }

    /// Returns the field name as the struct declares it.
    #[must_use]
    pub const fn name(&self) -> &'f str {
        self.name
    }

    /// Returns the value.
    #[must_use]
    pub const fn value(&self) -> &Value<'f> {
        &self.value
    }
}
