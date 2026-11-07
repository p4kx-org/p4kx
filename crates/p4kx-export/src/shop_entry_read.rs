// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_extraction::{Archive, ArchiveError, Entry};

use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDiagnosis;

/// A bounded shop source could not be read.
#[derive(Debug, thiserror::Error)]
pub(crate) enum ReadError {
    #[error("the file exceeds the read limit of {limit} bytes")]
    Limit { limit: u64 },
    #[error("the file cannot be opened: {0}")]
    Open(#[source] ArchiveError),
    #[error("the file cannot be read: {0}")]
    Read(#[source] io::Error),
}

impl ReadError {
    /// Returns the technical diagnosis of the actual bounded read failure.
    pub(crate) fn diagnosis(&self) -> SourceDiagnosis {
        let code = match self {
            Self::Limit { .. } => ExportDiagnosticCode::FieldWalkLimit,
            Self::Open(_) | Self::Read(_) => ExportDiagnosticCode::SourceReadFailure,
        };
        diagnosis(code, "content")
    }
}

/// Reads one shop source without accepting a payload beyond the declared limit.
pub(crate) fn read(archive: &Archive, entry: &Entry, limit: u64) -> Result<Vec<u8>, ReadError> {
    if entry.uncompressed_size() > limit {
        return Err(ReadError::Limit { limit });
    }
    let mut bytes = Vec::new();
    archive
        .open_entry(entry)
        .map_err(ReadError::Open)?
        .take(limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(ReadError::Read)?;
    if !u64::try_from(bytes.len()).is_ok_and(|length| length <= limit) {
        return Err(ReadError::Limit { limit });
    }
    Ok(bytes)
}

/// Retains the technical field associated with a shop source failure.
pub(crate) fn diagnosis(code: ExportDiagnosticCode, field: &str) -> SourceDiagnosis {
    SourceDiagnosis {
        code: Some(code),
        field: Some(field.to_owned()),
        source_type: Some("ShopSource".to_owned()),
        ..SourceDiagnosis::default()
    }
}

#[cfg(test)]
mod tests {
    use p4kx_testkit::{ArchiveBuilder, TestEntry};

    use super::*;

    type TestResult = Result<(), Box<dyn std::error::Error>>;

    fn opened(
        entry: TestEntry,
    ) -> Result<(tempfile::TempDir, Archive), Box<dyn std::error::Error>> {
        let folder = tempfile::tempdir()?;
        let path = folder.path().join("Data.p4k");
        ArchiveBuilder::new().entry(entry).write_to(&path)?;
        Ok((folder, Archive::open(path)?))
    }

    #[test]
    fn a_payload_at_the_limit_is_read_without_truncation() -> TestResult {
        let (_folder, archive) = opened(TestEntry::stored("shop.xml", "1234"))?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        assert_eq!(
            read(&archive, entry, 4)?,
            b"1234",
            "the entire bounded payload survives"
        );
        Ok(())
    }

    #[test]
    fn an_over_limit_entry_is_a_read_failure_with_a_budget_diagnosis() -> TestResult {
        let (_folder, archive) = opened(TestEntry::stored("shop.xml", "12345"))?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        let failure = read(&archive, entry, 4).expect_err("the declared source exceeds the limit");
        assert_eq!(
            failure.diagnosis().code,
            Some(ExportDiagnosticCode::FieldWalkLimit),
            "the read limit remains distinguishable from malformed XML"
        );
        Ok(())
    }

    #[test]
    fn corrupt_compression_is_not_reported_as_a_projection_failure() -> TestResult {
        let entry = TestEntry::zstd("shop.xml", "<Shop/>");
        let truncated = entry.payload().get(..4).ok_or("Zstd frame magic")?.to_vec();
        let entry = entry.with_payload(truncated);
        let (_folder, archive) = opened(entry)?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        let failure =
            read(&archive, entry, 64).expect_err("the canonical source cannot be decoded");
        assert_eq!(
            failure.diagnosis().code,
            Some(ExportDiagnosticCode::SourceReadFailure),
            "read and typed projection are separate stages"
        );
        Ok(())
    }
}
