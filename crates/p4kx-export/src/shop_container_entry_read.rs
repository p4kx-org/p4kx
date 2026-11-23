// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, Entry};

use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, EntryRead, EntryReadError};
use crate::entry_source::EntrySource;
use crate::export_diagnostic::ExportDiagnosticCode;
use crate::export_ledger::{ExportLedger, SourceDisposition};
use crate::shop_entry_read;

pub(super) fn read(
    container: &Archive,
    entry: &Entry,
    limit: u64,
    source: &EntrySource,
    ledger: &mut ExportLedger,
    capture: &EntryCapture<'_>,
) -> Result<Option<EntryRead>, CatalogError> {
    match shop_entry_read::read(capture, source, container, entry, limit)? {
        Ok(bytes) => Ok(Some(bytes)),
        Err(error) => {
            record_read_error(source.clone(), ledger, &error);
            Ok(None)
        }
    }
}

pub(super) fn record_read_error(
    source: EntrySource,
    ledger: &mut ExportLedger,
    error: &EntryReadError,
) {
    ledger.incomplete_inventory(crate::Kind::Shops);
    source.diagnosed(
        ledger,
        crate::Kind::Shops,
        SourceDisposition::ReadFailed,
        error.diagnosis(),
    );
}

pub(super) fn rejected(source: EntrySource, ledger: &mut ExportLedger, field: &'static str) {
    source.diagnosed(
        ledger,
        crate::Kind::Shops,
        SourceDisposition::ProjectionFailed,
        shop_entry_read::diagnosis(ExportDiagnosticCode::MalformedSource, field),
    );
}

pub(super) fn projected(
    source: EntrySource,
    ledger: &mut ExportLedger,
    malformed: bool,
    field: &'static str,
) {
    if malformed {
        rejected(source, ledger, field);
    } else {
        source.record(ledger, crate::Kind::Shops, SourceDisposition::Projected);
    }
}
