// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{ArchiveMetadataVersion, DecimalU64, MetadataReadError};
use schemars::JsonSchema;
use serde::Serialize;

use crate::{MediaError, MediaIssue, MediaWriter, Severity, Status, Subresource};

pub(crate) const DIAGNOSTIC_NAMESPACE: &str = "archive-metadata-diagnostic";

/// Structural failure of a lazy metadata operation, independently of raw retention.
#[derive(Debug, Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum ArchiveMetadataFailure {
    /// Recovery did not prove a global layout.
    UnavailableLayout,
    /// A declaration does not establish a bounded section.
    MalformedSection,
    /// A source header failed the shared parser.
    Parse,
    /// A record does not fit the declared section.
    MalformedRecord,
    /// Declared record count leaves unparsed directory bytes.
    UnparsedDirectoryTail,
}

/// Explicit failed operation retained in diagnostic exports.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[non_exhaustive]
pub struct ArchiveMetadataDiagnostic {
    /// Source-layout payload contract.
    pub version: ArchiveMetadataVersion,
    /// Stable affected operation or section namespace.
    pub operation: String,
    /// Structural failure category.
    pub failure: ArchiveMetadataFailure,
    /// Exact owning-source failure offset when supplied by the reader.
    pub offset: Option<DecimalU64>,
    /// Original unparsed-tail length when available.
    pub length: Option<DecimalU64>,
}

pub(crate) fn structural(
    cause: MetadataReadError,
    operation: &str,
    indices: Vec<u64>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let (failure, offset, length) = match &cause {
        MetadataReadError::UnavailableLayout => {
            (ArchiveMetadataFailure::UnavailableLayout, None, None)
        }
        MetadataReadError::MalformedSection => {
            (ArchiveMetadataFailure::MalformedSection, None, None)
        }
        MetadataReadError::Parse { offset, .. } => {
            (ArchiveMetadataFailure::Parse, Some(*offset), None)
        }
        MetadataReadError::MalformedRecord { offset } => {
            (ArchiveMetadataFailure::MalformedRecord, Some(*offset), None)
        }
        MetadataReadError::UnparsedDirectoryTail { offset, len } => (
            ArchiveMetadataFailure::UnparsedDirectoryTail,
            Some(*offset),
            Some(*len),
        ),
        _ => return Err(cause.into()),
    };
    let data = ArchiveMetadataDiagnostic {
        version: ArchiveMetadataVersion::V1,
        operation: operation.into(),
        failure,
        offset: offset.map(DecimalU64::new),
        length: length.map(DecimalU64::new),
    };
    let detail = match &cause {
        MetadataReadError::Parse { source, .. } => format!("{cause}: {source}"),
        _ => cause.to_string(),
    };
    writer.record(
        &Subresource::new(DIAGNOSTIC_NAMESPACE, indices),
        Status::Partial,
        &data,
        &[warning("archive.metadata-structure", operation, detail)],
    )
}

pub(crate) fn warning(code: &str, location: &str, detail: impl Into<String>) -> MediaIssue {
    MediaIssue {
        code: code.into(),
        location: location.into(),
        severity: Severity::Warning,
        detail: detail.into(),
    }
}
