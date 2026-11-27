// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataType;
use schemars::JsonSchema;
use serde::Serialize;

/// Closed wire names for original field element storage types.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
#[non_exhaustive]
pub enum SourceDataType {
    /// An original Boolean element.
    #[serde(rename = "bool")]
    Bool,
    /// An original signed 8-bit integer element.
    #[serde(rename = "int8")]
    Int8,
    /// An original signed 16-bit integer element.
    #[serde(rename = "int16")]
    Int16,
    /// An original signed 32-bit integer element.
    #[serde(rename = "int32")]
    Int32,
    /// An original signed 64-bit integer element.
    #[serde(rename = "int64")]
    Int64,
    /// An original unsigned 8-bit integer element.
    #[serde(rename = "uint8")]
    UInt8,
    /// An original unsigned 16-bit integer element.
    #[serde(rename = "uint16")]
    UInt16,
    /// An original unsigned 32-bit integer element.
    #[serde(rename = "uint32")]
    UInt32,
    /// An original unsigned 64-bit integer element.
    #[serde(rename = "uint64")]
    UInt64,
    /// An original string-table element.
    #[serde(rename = "string")]
    String,
    /// An original IEEE 754 binary32 element.
    #[serde(rename = "single")]
    Single,
    /// An original IEEE 754 binary64 element.
    #[serde(rename = "double")]
    Double,
    /// An original localization-key element.
    #[serde(rename = "locale")]
    Locale,
    /// An original GUID element.
    #[serde(rename = "guid")]
    Guid,
    /// An original enum-option element.
    #[serde(rename = "enum")]
    Enum,
    /// An original inline owned-class element.
    #[serde(rename = "class")]
    Class,
    /// An original strong pointer to an owned instance.
    #[serde(rename = "strongPointer")]
    StrongPointer,
    /// An original weak pointer to a Record or owned instance.
    #[serde(rename = "weakPointer")]
    WeakPointer,
    /// An original Reference to a Record.
    #[serde(rename = "reference")]
    Reference,
}

impl SourceDataType {
    pub(crate) const fn read(source: DataType) -> Self {
        match source {
            DataType::Bool => Self::Bool,
            DataType::Int8 => Self::Int8,
            DataType::Int16 => Self::Int16,
            DataType::Int32 => Self::Int32,
            DataType::Int64 => Self::Int64,
            DataType::UInt8 => Self::UInt8,
            DataType::UInt16 => Self::UInt16,
            DataType::UInt32 => Self::UInt32,
            DataType::UInt64 => Self::UInt64,
            DataType::String => Self::String,
            DataType::Single => Self::Single,
            DataType::Double => Self::Double,
            DataType::Locale => Self::Locale,
            DataType::Guid => Self::Guid,
            DataType::Enum => Self::Enum,
            DataType::Class => Self::Class,
            DataType::StrongPointer => Self::StrongPointer,
            DataType::WeakPointer => Self::WeakPointer,
            DataType::Reference => Self::Reference,
        }
    }
}
