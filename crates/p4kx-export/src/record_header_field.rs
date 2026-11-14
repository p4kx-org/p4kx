// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceLocation;

const RECORD_HEADER_NAMESPACE: &str = "dataforgeRecordHeader";

/// An actual original Record-header input, separate from literal body property names.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RecordHeaderField {
    Guid,
    RecordName,
    RecordIndex,
    SourceType,
    FileName,
    MetadataTag,
}

impl RecordHeaderField {
    pub(crate) fn location(self, record_index: u32) -> SourceLocation {
        SourceLocation::Subresource {
            namespace: RECORD_HEADER_NAMESPACE.to_owned(),
            indices: vec![u64::from(record_index)],
            field: self.name().to_owned(),
        }
    }

    const fn name(self) -> &'static str {
        match self {
            Self::Guid => "guid",
            Self::RecordName => "recordName",
            Self::RecordIndex => "recordIndex",
            Self::SourceType => "sourceType",
            Self::FileName => "fileName",
            Self::MetadataTag => "metadataTag",
        }
    }
}

/// A metadata operation whose actual input can be a raw header field or a normalized path.
#[derive(Debug, Clone, Copy)]
pub(crate) enum RecordHeaderInput {
    Field(RecordHeaderField),
    NormalizedPath,
}
