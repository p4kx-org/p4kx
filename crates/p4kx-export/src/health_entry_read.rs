// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};

use p4kx_assembly::LoadoutFileRead;
use p4kx_extraction::{Archive, ArchiveError, CancelToken, Entry};

use super::health_entry_frame::EntryCapture;
use super::health_entry_receipt::EntryReceipt;
use crate::{CatalogError, ExportAccountingError};
use p4kx_extraction::{SourceLocation, SourceUseRole};

use super::entry_selection_input::{EntrySelectionUse, SelectionSource};
use super::health_entry_selection::AuthoredEntryPath;

const READ_CHUNK_BYTES: usize = 8 * 1024;

/// A source read failure distinct from a fatal provenance-admission failure.
#[derive(Debug, thiserror::Error)]
pub(crate) enum EntryReadError {
    #[error("the file exceeds the read limit of {limit} bytes")]
    Limit { limit: u64 },
    #[error("the file cannot be opened: {0}")]
    Open(#[source] ArchiveError),
    #[error("the file cannot be read: {0}")]
    Read(#[source] io::Error),
}

/// Bytes paired privately to the exact Entry admitted before their actual read.
pub(crate) struct EntryRead {
    receipt: EntryReceipt,
    bytes: Vec<u8>,
    decoded_end_checked: bool,
}

impl EntryRead {
    pub(crate) fn from_observed(
        capture: &EntryCapture<'_>,
        read: LoadoutFileRead,
    ) -> Result<Self, CatalogError> {
        let receipt = EntryReceipt::loadout(capture, &read)?;
        let bytes = read
            .into_result()
            .map_err(CatalogError::LoadoutSourceEvidence)?;
        receipt.attempted_payload()?;
        Ok(Self {
            receipt,
            bytes,
            decoded_end_checked: false,
        })
    }

    pub(crate) fn read(
        receipt: EntryReceipt,
        archive: &Archive,
        entry: &Entry,
        limit: u64,
    ) -> Result<Result<Self, EntryReadError>, CatalogError> {
        Self::read_with(receipt, archive, entry, Some(limit), None)
    }

    pub(crate) fn read_all(
        receipt: EntryReceipt,
        archive: &Archive,
        entry: &Entry,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<Self, EntryReadError>, CatalogError> {
        Self::read_with(receipt, archive, entry, None, cancel)
    }

    fn read_with(
        receipt: EntryReceipt,
        archive: &Archive,
        entry: &Entry,
        limit: Option<u64>,
        cancel: Option<&CancelToken>,
    ) -> Result<Result<Self, EntryReadError>, CatalogError> {
        receipt.check_entry(archive, entry)?;
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        if let Some(limit) = limit
            && entry.uncompressed_size() > limit
        {
            return Ok(Err(EntryReadError::Limit { limit }));
        }
        receipt.attempted_payload()?;
        let reader = match archive.open_entry(entry) {
            Ok(reader) => reader,
            Err(error) => return Ok(Err(EntryReadError::Open(error))),
        };
        let mut bytes = Vec::new();
        let read = match limit {
            Some(limit) => reader
                .take(limit.saturating_add(1))
                .read_to_end(&mut bytes)
                .map(|_| ()),
            None => read_complete(
                reader.reject_overlong(),
                &mut bytes,
                entry.uncompressed_size(),
                cancel,
            )?,
        };
        if let Err(error) = read {
            return Ok(Err(EntryReadError::Read(error)));
        }
        if let Some(limit) = limit
            && !u64::try_from(bytes.len()).is_ok_and(|length| length <= limit)
        {
            return Ok(Err(EntryReadError::Limit { limit }));
        }
        Ok(Ok(Self {
            receipt,
            bytes,
            decoded_end_checked: limit.is_none(),
        }))
    }

    pub(crate) fn string_slot(
        &self,
        capture: &EntryCapture<'_>,
        offset: usize,
        value: &str,
    ) -> Result<AuthoredEntryPath, CatalogError> {
        self.receipt.check_frame(&capture.frame())?;
        let end = offset
            .checked_add(value.len())
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        let original = self
            .bytes
            .get(offset..end)
            .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
        if !std::ptr::eq(original, value.as_bytes())
            || (offset > 0 && self.bytes.get(offset - 1) != Some(&0))
            || (end < self.bytes.len() && self.bytes.get(end) != Some(&0))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let offset =
            u64::try_from(offset).map_err(|_| ExportAccountingError::ConflictingSourceDatum)?;
        let location = SourceLocation::Subresource {
            namespace: "shopSoc".to_owned(),
            indices: vec![offset],
            field: "string".to_owned(),
        };
        let origin = self.receipt.payload_entry()?.clone();
        AuthoredEntryPath::captured(
            capture,
            value,
            vec![
                EntrySelectionUse {
                    source: SelectionSource::Entry(origin.clone()),
                    location: location.clone(),
                    role: SourceUseRole::Payload,
                },
                EntrySelectionUse {
                    source: SelectionSource::Entry(origin),
                    location,
                    role: SourceUseRole::Conversion,
                },
            ],
        )
    }

    pub(crate) fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub(super) fn complete_bytes(&self) -> Result<&[u8], ExportAccountingError> {
        if !self.decoded_end_checked {
            return Err(ExportAccountingError::ConflictingSourceDatum);
        }
        Ok(&self.bytes)
    }

    pub(crate) const fn receipt(&self) -> &EntryReceipt {
        &self.receipt
    }
}

fn read_complete(
    mut reader: impl Read,
    bytes: &mut Vec<u8>,
    declared_size: u64,
    cancel: Option<&CancelToken>,
) -> Result<io::Result<()>, CatalogError> {
    let mut chunk = [0_u8; READ_CHUNK_BYTES];
    loop {
        if cancel.is_some_and(CancelToken::is_cancelled) {
            return Err(CatalogError::Cancelled);
        }
        let count = match reader.read(&mut chunk) {
            Ok(0) => return Ok(Ok(())),
            Ok(count) => count,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Ok(Err(error)),
        };
        let Some(data) = chunk.get(..count) else {
            return Ok(Err(io::Error::other("Entry reader exceeded its buffer")));
        };
        bytes
            .try_reserve(data.len())
            .map_err(|source| CatalogError::EntryPayloadAllocation {
                size: declared_size,
                source,
            })?;
        bytes.extend_from_slice(data);
    }
}
