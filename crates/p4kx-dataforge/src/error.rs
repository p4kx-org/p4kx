// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_foundation::ParseError;
use thiserror::Error;

/// A DataForge that cannot be read, or a value inside it that is invalid.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DataForgeError {
    /// A canonical-read observer stopped the operation for its caller-owned reason.
    #[error("the canonical-read observer stopped the operation")]
    ObservationStopped,
    /// The header names a version this reader does not know.
    #[error(
        "DataForge version {version} is not supported; this version of P4KX reads versions 6 and 8"
    )]
    UnsupportedVersion {
        /// The version in the header.
        version: u32,
    },
    /// The input ends inside the named part.
    #[error("the DataForge is truncated in its {part}")]
    Truncated {
        /// The header or table that was cut off.
        part: &'static str,
        /// The failed read.
        #[source]
        source: ParseError,
    },
    /// The named part is larger than this platform can address.
    #[error("the {part} of the DataForge is too large")]
    TooLarge {
        /// The table whose size overflows.
        part: &'static str,
    },
    /// A table row refers to a row of another table that does not exist.
    #[error(
        "{part} {index} of the DataForge refers to {target} {target_index}, which does not exist"
    )]
    IndexOutOfRange {
        /// The table holding the reference.
        part: &'static str,
        /// The row holding the reference.
        index: usize,
        /// The table referred to.
        target: &'static str,
        /// The missing row.
        target_index: u64,
    },
    /// Two data mappings place instances of the same struct.
    #[error("struct {struct_index} of the DataForge has more than one data mapping")]
    DuplicateMapping {
        /// The struct mapped twice.
        struct_index: u32,
    },
    /// The instance data does not fill the input behind the tables exactly.
    #[error(
        "the instances of the DataForge take {expected} bytes, but {actual} bytes follow its tables"
    )]
    InstanceSize {
        /// The size the data mappings give.
        expected: u64,
        /// The size left in the input.
        actual: u64,
    },
    /// A string offset is not the start of a terminated UTF-8 string in its table.
    #[error("offset {offset} in the {table} table of the DataForge is not a valid string")]
    InvalidString {
        /// The text or the blob table.
        table: &'static str,
        /// The offset into the table.
        offset: u32,
    },
    /// A struct inherits from itself through its chain of parents.
    #[error("struct {struct_index} of the DataForge inherits from itself")]
    StructCycle {
        /// The struct whose parents were followed.
        struct_index: u32,
    },
    /// A struct has more ancestors than [`MAX_INHERITANCE_DEPTH`](crate::MAX_INHERITANCE_DEPTH).
    #[error("struct {struct_index} of the DataForge has more than {limit} ancestors")]
    InheritanceTooDeep {
        /// The struct whose parents were followed.
        struct_index: u32,
        /// The largest number of structs in one chain.
        limit: usize,
    },
    /// A struct and its ancestors declare more fields than
    /// [`MAX_RECORD_FIELDS`](crate::MAX_RECORD_FIELDS).
    #[error("struct {struct_index} of the DataForge has {count} fields, more than {limit}")]
    TooManyFields {
        /// The struct whose fields were counted.
        struct_index: u32,
        /// The number of fields, inherited ones included.
        count: usize,
        /// The largest number of fields that is read.
        limit: usize,
    },
    /// The fields of a struct do not fill its declared instance size.
    #[error(
        "struct {struct_index} of the DataForge declares {declared} bytes per instance, but its fields take {fields}"
    )]
    StructLayout {
        /// The struct whose fields were laid out.
        struct_index: u32,
        /// The instance size the struct table gives.
        declared: u32,
        /// The size of the fields up to and including this struct.
        fields: u64,
    },
    /// A property has a data type code this reader does not know.
    #[error("the DataForge uses the unknown data type {code:#06x}")]
    UnknownDataType {
        /// The raw code.
        code: u16,
    },
    /// A Record tree would hold more than [`MAX_RECORD_VALUES`](crate::MAX_RECORD_VALUES)
    /// values.
    #[error("the Record holds more than {limit} values")]
    RecordTooLarge {
        /// The most values one Record tree may hold.
        limit: usize,
    },
}
