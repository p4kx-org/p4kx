// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{
    DataForge, RecordJsonBytes, RecordJsonRecordField, RecordJsonSource, RecordJsonUse,
    RecordJsonUseRole,
};

use crate::{SourceLocation, SourceUseRole};

/// The source coordinate and contribution role of an observed Record JSON value.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct RecordJsonLocation {
    /// Original data or selector coordinate.
    pub location: SourceLocation,
    /// How this use contributed to the emitted value.
    pub role: SourceUseRole,
}

impl RecordJsonLocation {
    /// Addresses the original text borrowed from this DataForge.
    #[must_use]
    pub fn borrowed_text(forge: &DataForge<'_>, text: &str) -> Option<SourceLocation> {
        let span = forge.original_text_span(text)?;
        Self::of(RecordJsonSource::Bytes {
            start: span.start,
            end: span.end,
            kind: RecordJsonBytes::Text,
        })
        .map(|source| source.location)
    }

    /// Resolves one Record JSON observation to its exact original input coordinate.
    #[must_use]
    pub fn of(source: RecordJsonSource<'_>) -> Option<Self> {
        match source {
            RecordJsonSource::Record { index, field } => Self::record(index, field),
            RecordJsonSource::StructName { index } => Some(Self::subresource(
                "dataforgeStructDefinition",
                "name",
                vec![u64::from(index)],
                SourceUseRole::Payload,
            )),
            RecordJsonSource::PropertyName { index } => Some(Self::subresource(
                "dataforgePropertyDefinition",
                "name",
                vec![index as u64],
                SourceUseRole::Selector,
            )),
            RecordJsonSource::PropertyStorage { index, target_used } => Some(Self::subresource(
                "dataforgePropertyDefinition",
                if target_used {
                    "targetedInstanceLayout"
                } else {
                    "instanceLayout"
                },
                vec![index as u64],
                SourceUseRole::Conversion,
            )),
            RecordJsonSource::InstancePath { record, ordinals } => {
                let mut indices = vec![u64::from(record)];
                indices.extend_from_slice(ordinals);
                Some(Self::subresource(
                    "dataforgeOwnedInstance",
                    "path",
                    indices,
                    SourceUseRole::Selector,
                ))
            }
            RecordJsonSource::Bytes { start, end, kind } => Self::bytes(start, end, kind),
            _ => None,
        }
    }

    /// Resolves an observed reference contributor with its role for the emitted member.
    #[must_use]
    pub fn of_use(observed: &RecordJsonUse) -> Option<Self> {
        let role = Self::use_role(observed.role)?;
        let mut located = Self::of(observed.source)?;
        located.role = role;
        Some(located)
    }

    /// Maps the contribution role assigned by the Record JSON observer.
    #[must_use]
    pub const fn use_role(role: RecordJsonUseRole) -> Option<SourceUseRole> {
        match role {
            RecordJsonUseRole::Payload => Some(SourceUseRole::Payload),
            RecordJsonUseRole::Selector => Some(SourceUseRole::Selector),
            RecordJsonUseRole::Conversion => Some(SourceUseRole::Conversion),
            _ => None,
        }
    }

    fn record(index: u32, field: RecordJsonRecordField) -> Option<Self> {
        let (field, role) = match field {
            RecordJsonRecordField::Guid => ("guid", SourceUseRole::Payload),
            RecordJsonRecordField::FileName => ("fileName", SourceUseRole::Conversion),
            RecordJsonRecordField::Name => ("recordName", SourceUseRole::Conversion),
            RecordJsonRecordField::Tag => ("tag", SourceUseRole::Conversion),
            RecordJsonRecordField::Struct => ("sourceType", SourceUseRole::Selector),
            _ => return None,
        };
        Some(Self {
            location: SourceLocation::DataForge {
                record_index: index,
                field: field.into(),
            },
            role,
        })
    }

    fn bytes(start: u64, end: u64, kind: RecordJsonBytes) -> Option<Self> {
        let (label, role) = match kind {
            RecordJsonBytes::Slot => ("slot", SourceUseRole::Payload),
            RecordJsonBytes::ArrayHeader => ("arrayHeader", SourceUseRole::Selector),
            RecordJsonBytes::PointerSelector => ("pointerSelector", SourceUseRole::Selector),
            RecordJsonBytes::ArrayElement => ("arrayElement", SourceUseRole::Payload),
            RecordJsonBytes::Text => ("text", SourceUseRole::Payload),
            _ => return None,
        };
        Some(Self::subresource(
            "dataforgeDecodedBytes",
            label,
            vec![start, end],
            role,
        ))
    }

    fn subresource(namespace: &str, field: &str, indices: Vec<u64>, role: SourceUseRole) -> Self {
        Self {
            location: SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: field.to_owned(),
            },
            role,
        }
    }
}
