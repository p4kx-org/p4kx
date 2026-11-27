// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Guid, Record};

use crate::CatalogError;
use crate::catalog_query_session::checkpoint;

/// Selects physical Records within the queried dataset and supplied DataForge.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub enum CatalogRecordSelection {
    /// All Records admitted to the queried dataset by canonical catalog policy.
    All,
    /// All admitted Records with this GUID, including duplicates.
    Guid(Guid),
    /// The Record with this physical index in the supplied DataForge.
    RecordIndex(u32),
}

impl CatalogRecordSelection {
    pub(crate) fn for_each<'source>(
        self,
        forge: &'source DataForge<'_>,
        cancel: Option<&CancelToken>,
        read: impl FnMut(&Record<'source>) -> Result<(), CatalogError>,
    ) -> Result<(), CatalogError> {
        self.try_for_each(forge, cancel, read)
    }

    pub(crate) fn try_for_each<'source, E: From<CatalogError>>(
        self,
        forge: &'source DataForge<'_>,
        cancel: Option<&CancelToken>,
        mut read: impl FnMut(&Record<'source>) -> Result<(), E>,
    ) -> Result<(), E> {
        if let Self::Guid(guid) = self
            && let Some(records) = forge.records_by_guid_if_indexed(guid)
        {
            checkpoint(cancel)?;
            for record in records {
                checkpoint(cancel)?;
                read(&record)?;
            }
            return Ok(());
        }
        match self {
            Self::RecordIndex(index) => {
                if let Some(record) = forge.record(index) {
                    read(&record)?;
                }
            }
            Self::All | Self::Guid(_) => {
                for record in forge.records() {
                    checkpoint(cancel)?;
                    if matches!(self, Self::Guid(guid) if record.guid() != guid) {
                        continue;
                    }
                    read(&record)?;
                }
            }
        }
        Ok(())
    }
}

pub(crate) enum RecordVisit<'selection> {
    Records(CatalogRecordSelection),
    Indices {
        indices: &'selection [u32],
        missing: &'selection mut Vec<u32>,
    },
}

impl From<CatalogRecordSelection> for RecordVisit<'_> {
    fn from(selection: CatalogRecordSelection) -> Self {
        Self::Records(selection)
    }
}

impl RecordVisit<'_> {
    pub(crate) fn try_for_each<'source, E: From<CatalogError>>(
        self,
        forge: &'source DataForge<'_>,
        cancel: Option<&CancelToken>,
        mut read: impl FnMut(&Record<'source>) -> Result<(), E>,
    ) -> Result<(), E> {
        match self {
            Self::Records(selection) => selection.try_for_each(forge, cancel, read),
            Self::Indices { indices, missing } => {
                checkpoint(cancel)?;
                let mut indices = indices.to_vec();
                indices.sort_unstable();
                indices.dedup();
                for index in indices {
                    checkpoint(cancel)?;
                    if let Some(record) = forge.record(index) {
                        read(&record)?;
                    } else {
                        missing.push(index);
                    }
                }
                Ok(())
            }
        }
    }
}

/// Physical Records that could not be read fully, or were skipped by selection policy.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
#[non_exhaustive]
pub struct CatalogRecordCoverage {
    /// Physical indices whose headers could not be classified.
    pub unreadable_headers: Vec<u32>,
    /// Selected physical indices whose bodies could not be read.
    pub unreadable_bodies: Vec<u32>,
    /// Selected physical indices with recoverable field projection issues.
    pub incomplete_fields: Vec<u32>,
    /// The number of development leftovers excluded by catalog policy.
    pub skipped_leftovers: usize,
}
