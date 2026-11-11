// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DataType;
use schemars::JsonSchema;
use serde::Serialize;

/// Closed wire names for original field element storage types.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
pub(crate) enum SourceDataType {
    #[serde(rename = "bool")]
    Bool,
    #[serde(rename = "int8")]
    Int8,
    #[serde(rename = "int16")]
    Int16,
    #[serde(rename = "int32")]
    Int32,
    #[serde(rename = "int64")]
    Int64,
    #[serde(rename = "uint8")]
    UInt8,
    #[serde(rename = "uint16")]
    UInt16,
    #[serde(rename = "uint32")]
    UInt32,
    #[serde(rename = "uint64")]
    UInt64,
    #[serde(rename = "string")]
    String,
    #[serde(rename = "single")]
    Single,
    #[serde(rename = "double")]
    Double,
    #[serde(rename = "locale")]
    Locale,
    #[serde(rename = "guid")]
    Guid,
    #[serde(rename = "enum")]
    Enum,
    #[serde(rename = "class")]
    Class,
    #[serde(rename = "strongPointer")]
    StrongPointer,
    #[serde(rename = "weakPointer")]
    WeakPointer,
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
