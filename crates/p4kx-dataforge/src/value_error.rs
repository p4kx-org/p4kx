// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use thiserror::Error;

use crate::Guid;

/// Why one value of a Record cannot be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum ValueError {
    /// A string, localisation key or enum value is not a valid string offset.
    #[error("offset {offset} in the {table} table is not a valid string")]
    InvalidString {
        /// The table the offset points into.
        table: &'static str,
        /// The offset.
        offset: u32,
    },
    /// An enum field names an enum that does not exist.
    #[error("the field refers to enum {index}, which does not exist")]
    UnknownEnum {
        /// The enum index the property gives.
        index: u16,
    },
    /// A field declaration names a struct that does not exist.
    #[error("the field refers to struct {index}, which does not exist")]
    UnknownStruct {
        /// The struct index the property gives.
        index: u16,
    },
    /// A bool byte is neither 0 nor 1.
    #[error("the bool byte {byte} is neither 0 nor 1")]
    InvalidBool {
        /// The stored byte.
        byte: u8,
    },
    /// A strong or weak pointer names a struct or an instance that does not exist.
    #[error(
        "the pointer refers to instance {instance_index} of struct {struct_index}, which does not exist"
    )]
    InvalidPointer {
        /// The struct index of the pointer.
        struct_index: u32,
        /// The instance index of the pointer.
        instance_index: u32,
    },
    /// The elements of an array lie outside the values stored for its type.
    #[error(
        "the array of {count} values from index {first} lies outside the {pool} of {len} values"
    )]
    ArrayOutOfRange {
        /// The index of the first element.
        first: u32,
        /// The number of elements.
        count: u32,
        /// The number of values stored for the type.
        len: u64,
        /// Where the values are stored, e.g. `int32 pool` or `instance block`.
        pool: &'static str,
    },
    /// An instance is owned a second time within one Record, e.g. by a cycle of strong
    /// pointers.
    #[error(
        "instance {instance_index} of struct {struct_index} is owned more than once in the Record"
    )]
    OwnedTwice {
        /// The struct of the instance.
        struct_index: u32,
        /// The instance.
        instance_index: u32,
    },
    /// A non-array inline class contains the same inline type on its active path.
    #[error("inline class {struct_index} contains itself")]
    InlineClassCycle {
        /// The original target index selected by the cyclic class declaration.
        struct_index: u32,
    },
    /// A Reference names a Record that does not exist.
    #[error("the Reference refers to the Record {guid}, which does not exist")]
    DanglingReference {
        /// The GUID of the missing Record.
        guid: Guid,
    },
    /// A weak pointer refers to an instance that is neither inside the Record nor a Record.
    #[error(
        "the weak pointer refers to instance {instance_index} of struct {struct_index}, which lies outside the Record"
    )]
    OutsideRecord {
        /// The struct index of the pointer.
        struct_index: u32,
        /// The instance index of the pointer.
        instance_index: u32,
    },
    /// A strong pointer refers to an instance of a struct that neither is nor derives from the
    /// struct the field declares.
    #[error(
        "the pointer refers to struct {struct_index}, which does not derive from struct {declared}, the type of the field"
    )]
    UnexpectedPointerType {
        /// The struct index of the pointer.
        struct_index: u32,
        /// The struct the field declares.
        declared: u32,
    },
    /// A Reference a Query follows refers to a Record of a struct that neither is nor derives
    /// from the struct the field declares.
    #[error(
        "the Reference refers to the Record {guid} of struct {struct_index}, which does not derive from struct {declared}, the type of the field"
    )]
    UnexpectedReferenceType {
        /// The GUID of the Record.
        guid: Guid,
        /// The struct index of the Record.
        struct_index: u32,
        /// The struct the field declares.
        declared: u32,
    },
    /// The paths of a Query reach more array elements in one Record than
    /// [`MAX_PATH_VALUES`](crate::query::MAX_PATH_VALUES).
    #[error("the paths of the Record reach more than {limit} array elements")]
    PathTooWide {
        /// The most elements the paths of one Record may reach.
        limit: usize,
    },
    /// An instance is shorter than the layout a Query path reads it with.
    #[error(
        "the instance of struct {struct_index} has {size} bytes, but the path reads up to byte {needed}"
    )]
    InstanceTooShort {
        /// The struct of the instance.
        struct_index: u32,
        /// The bytes the instance has.
        size: u64,
        /// The end of the bytes the path reads.
        needed: u64,
    },
}

impl ValueError {
    /// Returns the GUID of the missing Record for a [`ValueError::DanglingReference`], `None` for
    /// any other error.
    #[must_use]
    pub const fn dangling_guid(&self) -> Option<Guid> {
        match self {
            Self::DanglingReference { guid } => Some(*guid),
            _ => None,
        }
    }
}
