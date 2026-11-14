// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry};

use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, EntryDiagnosis, EntryRead, EntryReadError};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;

impl EntryReadError {
    /// Returns the technical diagnosis of the actual Entry read failure.
    pub(crate) const fn diagnosis(&self) -> EntryDiagnosis {
        let code = match self {
            Self::Limit { .. } => ExportDiagnosticCode::FieldWalkLimit,
            Self::Open(_) | Self::Read(_) => ExportDiagnosticCode::SourceReadFailure,
        };
        diagnosis(code, "content")
    }
}

/// Reads the bytes with the same original Entry admission as the source observation.
pub(crate) fn read(
    capture: &EntryCapture<'_>,
    source: &EntrySource,
    archive: &Archive,
    entry: &Entry,
    limit: u64,
) -> Result<Result<EntryRead, EntryReadError>, CatalogError> {
    source.receipt().check_capture(capture)?;
    EntryRead::read(source.receipt().clone(), archive, entry, limit)
}

/// Reads the complete Entry with the same admission as the source observation.
pub(crate) fn read_all(
    capture: &EntryCapture<'_>,
    source: &EntrySource,
    archive: &Archive,
    entry: &Entry,
    cancel: Option<&CancelToken>,
) -> Result<Result<EntryRead, EntryReadError>, CatalogError> {
    source.receipt().check_capture(capture)?;
    EntryRead::read_all(source.receipt().clone(), archive, entry, cancel)
}

/// Returns the tool-defined field associated with a shop source failure.
pub(crate) const fn diagnosis(code: ExportDiagnosticCode, field: &'static str) -> EntryDiagnosis {
    EntryDiagnosis::technical(code, field, "ShopSource")
}

#[cfg(test)]
mod tests {
    use std::io::{ErrorKind, Read};

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

    fn read_test(
        archive: &Archive,
        entry: &Entry,
        limit: u64,
    ) -> Result<Result<Vec<u8>, EntryReadError>, CatalogError> {
        crate::catalog_read::original::with_entry_capture(archive, |capture| {
            let source = EntrySource::outer(capture, archive, entry)?;
            Ok(read(capture, &source, archive, entry, limit)?.map(|read| read.bytes().to_vec()))
        })
    }

    #[test]
    fn a_payload_at_the_limit_is_read_without_truncation() -> TestResult {
        let (_folder, archive) = opened(TestEntry::stored("shop.xml", "1234"))?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        assert_eq!(
            read_test(&archive, entry, 4)??,
            b"1234",
            "the entire bounded payload survives"
        );
        Ok(())
    }

    #[test]
    fn an_over_limit_entry_is_a_read_failure_with_a_budget_diagnosis() -> TestResult {
        let (_folder, archive) = opened(TestEntry::stored("shop.xml", "12345"))?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        let failure =
            read_test(&archive, entry, 4)?.expect_err("the declared source exceeds the limit");
        assert_eq!(
            failure.diagnosis().code,
            Some(ExportDiagnosticCode::FieldWalkLimit),
            "the read limit remains distinguishable from malformed XML"
        );
        Ok(())
    }

    fn read_complete_test(
        archive: &Archive,
        entry: &Entry,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<Vec<u8>, EntryReadError>, CatalogError> {
        crate::catalog_read::original::with_entry_capture(archive, |capture| {
            let source = EntrySource::outer(capture, archive, entry)?;
            Ok(read_all(capture, &source, archive, entry, cancel)?
                .map(|read| read.bytes().to_vec()))
        })
    }

    #[test]
    fn complete_read_rejects_an_underdeclared_payload_with_a_valid_prefix() -> TestResult {
        let (_folder, archive) =
            opened(TestEntry::deflated("shop.json", "{} ").with_uncompressed_size(2))?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        let mut prefix = Vec::new();
        archive.open_entry(entry)?.read_to_end(&mut prefix)?;
        assert_eq!(
            prefix, b"{}",
            "the ordinary reader returns the declared prefix"
        );
        let failure = read_complete_test(&archive, entry, None)?
            .expect_err("a complete read must reject the source length mismatch");
        assert!(
            matches!(&failure, EntryReadError::Read(error) if error.kind() == ErrorKind::InvalidData),
            "{failure:?}"
        );
        assert_eq!(
            failure.diagnosis().code,
            Some(ExportDiagnosticCode::SourceReadFailure),
            "the mismatch is a read failure"
        );
        Ok(())
    }

    #[test]
    fn complete_read_does_not_return_an_inventory_after_cancellation() -> TestResult {
        let (_folder, archive) = opened(TestEntry::stored("shop.json", "{}"))?;
        let entry = archive.entries().first().ok_or("source Entry")?;
        let cancel = CancelToken::new();
        cancel.cancel();
        assert!(
            matches!(
                read_complete_test(&archive, entry, Some(&cancel)),
                Err(CatalogError::Cancelled)
            ),
            "cancellation refuses the read before an EntryRead is returned"
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
            read_test(&archive, entry, 64)?.expect_err("the canonical source cannot be decoded");
        assert_eq!(
            failure.diagnosis().code,
            Some(ExportDiagnosticCode::SourceReadFailure),
            "read and typed projection are separate stages"
        );
        Ok(())
    }
}
