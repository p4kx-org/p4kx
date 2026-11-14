// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// One actual original field or array edge beneath a root or owned value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum OriginalValueEdge {
    Field { ordinal: u64 },
    Element { ordinal: u64 },
}

/// A completed actual owned container, independent of its child count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OriginalContainerKind {
    Struct,
    Array,
}

/// A distinct original logical fact observed at one actual value occurrence.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OriginalValuePart {
    Value,
    FieldName,
    DataType,
    IsArray,
    Target,
    ConcreteType,
    Error(OriginalErrorParameter),
}

/// A retained original error parameter, excluding fixed policy and label controls.
#[derive(Debug, Clone, Copy)]
pub(crate) enum OriginalErrorParameter {
    Offset,
    Byte,
    StructIndex,
    InstanceIndex,
    First,
    Count,
    Len,
    Guid,
}

impl OriginalValuePart {
    pub(super) const fn is_declaration(self) -> bool {
        matches!(
            self,
            Self::FieldName | Self::DataType | Self::IsArray | Self::Target
        )
    }

    pub(super) const fn label(self) -> &'static str {
        match self {
            Self::Value => "$value",
            Self::FieldName => "$fieldName",
            Self::DataType => "$dataType",
            Self::IsArray => "$isArray",
            Self::Target => "$target",
            Self::ConcreteType => "$type",
            Self::Error(parameter) => parameter.label(),
        }
    }
}

impl OriginalErrorParameter {
    const fn label(self) -> &'static str {
        match self {
            Self::Offset => "$error.offset",
            Self::Byte => "$error.byte",
            Self::StructIndex => "$error.structIndex",
            Self::InstanceIndex => "$error.instanceIndex",
            Self::First => "$error.first",
            Self::Count => "$error.count",
            Self::Len => "$error.len",
            Self::Guid => "$error.guid",
        }
    }
}
