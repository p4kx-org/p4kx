// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, Entry};

use crate::CatalogError;
use crate::catalog_read::original::{
    EntryCapture, EntryDiagnosis, EntryJson, EntryRead, EntryReadError,
};
use crate::catalog_read::parsed_entry_json::ParsedEntryJson;
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::SourceDisposition;

/// The verified bytes from one original loose JSON Entry.
pub(crate) struct LooseJson {
    read: EntryRead,
}

pub(crate) struct LooseJsonFailure {
    pub(crate) code: ExportDiagnosticCode,
    pub(crate) disposition: SourceDisposition,
}

impl LooseJsonFailure {
    pub(crate) const fn diagnosis(&self, source_type: &'static str) -> EntryDiagnosis {
        EntryDiagnosis::technical(self.code, "content", source_type)
    }

    pub(crate) const fn malformed() -> Self {
        Self {
            code: ExportDiagnosticCode::MalformedSource,
            disposition: SourceDisposition::ProjectionFailed,
        }
    }

    const fn read(code: ExportDiagnosticCode) -> Self {
        Self {
            code,
            disposition: SourceDisposition::ReadFailed,
        }
    }
}

impl LooseJson {
    /// Reads exactly the original decoded length with checksum verification and cancellation.
    pub(crate) fn read(
        archive: &Archive,
        entry: &Entry,
        source: &EntrySource,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<Self, LooseJsonFailure>, CatalogError> {
        if entry.uncompressed_size() > isize::MAX as u64 {
            return Ok(Err(LooseJsonFailure::read(
                ExportDiagnosticCode::FieldWalkLimit,
            )));
        }
        let read = EntryRead::read_verified(source.receipt().clone(), archive, entry, cancel)?;
        Ok(read
            .and_then(|read| {
                if u64::try_from(read.bytes().len()).ok() != Some(entry.uncompressed_size()) {
                    return Err(EntryReadError::Read(std::io::Error::from(
                        std::io::ErrorKind::UnexpectedEof,
                    )));
                }
                Ok(Self { read })
            })
            .map_err(|error| {
                let code = match error {
                    EntryReadError::Limit { .. } => ExportDiagnosticCode::FieldWalkLimit,
                    EntryReadError::Open(_) | EntryReadError::Read(_) => {
                        ExportDiagnosticCode::SourceReadFailure
                    }
                };
                LooseJsonFailure::read(code)
            }))
    }

    /// Parses and visits every original token without a fixed document size or depth quota.
    pub(crate) fn parse(
        &self,
        capture: &EntryCapture<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<ParsedEntryJson<'_>, LooseJsonFailure>, CatalogError> {
        let Ok(root) = EntryJson::parse(&self.read, capture)? else {
            return Ok(Err(LooseJsonFailure::malformed()));
        };
        Ok(Ok(ParsedEntryJson::read(root, cancel)?))
    }
}
