// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::path::Path;

use p4kx_extraction::{CancelToken, DataForge};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::spool::{IndexedRowSpoolWriter, RowSpool, RowSpoolError, RowSpoolWriter, checkpoint};
use crate::catalog_table::DatasetAvailability;
use crate::{CatalogError, ExportAccountingError};

/// A lazily opened private spool for an ordered catalog table.
pub(crate) struct StagedCatalogRows<'a> {
    directory: &'a Path,
    cancel: Option<&'a CancelToken>,
    writer: Option<RowSpoolWriter>,
}

impl<'a> StagedCatalogRows<'a> {
    pub(crate) const fn new(directory: &'a Path, cancel: Option<&'a CancelToken>) -> Self {
        Self {
            directory,
            cancel,
            writer: None,
        }
    }

    pub(crate) fn append(&mut self, row: &impl Serialize) -> Result<(), RowSpoolError> {
        let writer = if let Some(writer) = &mut self.writer {
            writer
        } else {
            checkpoint(self.cancel)?;
            self.writer.insert(RowSpoolWriter::new(self.directory)?)
        };
        writer.append(row, self.cancel)
    }

    pub(crate) fn finish(
        self,
        availability: DatasetAvailability,
    ) -> Result<StagedCatalogTable, RowSpoolError> {
        let spool = if let Some(writer) = self.writer {
            Some(writer.finish(self.cancel)?)
        } else {
            checkpoint(self.cancel)?;
            None
        };
        Ok(StagedCatalogTable::new(availability, spool, self.cancel))
    }
}

/// A private spool that admits rows in physical order and emits them stably by key.
pub(crate) struct IndexedCatalogRows<'stage, Key> {
    directory: &'stage Path,
    cancel: Option<&'stage CancelToken>,
    writer: Option<IndexedRowSpoolWriter<Key>>,
}

impl<'stage, Key: Ord> IndexedCatalogRows<'stage, Key> {
    pub(crate) const fn new(directory: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            directory,
            cancel,
            writer: None,
        }
    }

    pub(crate) fn append(&mut self, key: Key, row: &impl Serialize) -> Result<(), CatalogError> {
        let writer = if let Some(writer) = &mut self.writer {
            writer
        } else {
            checkpoint(self.cancel)?;
            self.writer
                .insert(IndexedRowSpoolWriter::new(self.directory)?)
        };
        writer
            .append(key, row, self.cancel)
            .map_err(CatalogError::from)
    }

    pub(crate) fn finish(
        self,
        forge: &DataForge<'_>,
        source_types: &[&str],
    ) -> Result<StagedCatalogTable, CatalogError> {
        let availability = if self.writer.is_some() {
            DatasetAvailability::Available
        } else {
            DatasetAvailability::from_source_declarations(forge, |name| {
                source_types.contains(&name)
            })
        };
        self.finish_with_availability(availability)
    }

    pub(crate) fn finish_with_availability(
        self,
        availability: DatasetAvailability,
    ) -> Result<StagedCatalogTable, CatalogError> {
        let spool = self
            .writer
            .map(|writer| writer.finish(self.cancel))
            .transpose()?;
        checkpoint(self.cancel)?;
        Ok(StagedCatalogTable::new(availability, spool, self.cancel))
    }
}

/// An ordered spool for rows keyed by original Record and component positions.
pub(crate) struct OrderedComponentRows<'stage> {
    rows: StagedCatalogRows<'stage>,
    last_key: Option<(Option<u32>, Option<u32>)>,
}

impl<'stage> OrderedComponentRows<'stage> {
    pub(crate) const fn new(directory: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            rows: StagedCatalogRows::new(directory, cancel),
            last_key: None,
        }
    }

    pub(crate) fn append(
        &mut self,
        key: (Option<u32>, Option<u32>),
        row: &impl Serialize,
    ) -> Result<(), CatalogError> {
        if self.last_key.is_some_and(|previous| previous > key) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.rows.append(row)?;
        self.last_key = Some(key);
        Ok(())
    }

    pub(crate) fn finish(
        self,
        forge: &DataForge<'_>,
        source_types: &[&str],
    ) -> Result<StagedCatalogTable, CatalogError> {
        let availability = if self.last_key.is_some() {
            DatasetAvailability::Available
        } else {
            DatasetAvailability::from_source_declarations(forge, |name| {
                source_types.contains(&name)
            })
        };
        Ok(self.rows.finish(availability)?)
    }
}

/// One catalog table whose ordered rows are prepared in a private spool.
#[derive(Debug)]
pub(crate) struct StagedCatalogTable {
    availability: DatasetAvailability,
    spool: Option<RowSpool>,
    cancel: Option<CancelToken>,
    failure: RefCell<Option<RowSpoolError>>,
}

impl StagedCatalogTable {
    pub(crate) const fn empty(availability: DatasetAvailability) -> Self {
        Self {
            availability,
            spool: None,
            cancel: None,
            failure: RefCell::new(None),
        }
    }

    pub(crate) fn new(
        availability: DatasetAvailability,
        spool: Option<RowSpool>,
        cancel: Option<&CancelToken>,
    ) -> Self {
        Self {
            availability,
            spool,
            cancel: cancel.cloned(),
            failure: RefCell::new(None),
        }
    }

    pub(crate) const fn len(&self) -> usize {
        match &self.spool {
            Some(spool) => spool.rows(),
            None => 0,
        }
    }

    pub(crate) const fn stored_bytes(&self) -> u64 {
        match &self.spool {
            Some(spool) => spool.stored_bytes(),
            None => 0,
        }
    }

    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.failure.take()
    }
}

impl Serialize for StagedCatalogTable {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let mut table = serializer.serialize_struct("CatalogTable", 2)?;
        table.serialize_field("availability", &self.availability)?;
        if let Some(spool) = &self.spool {
            let rows = spool.serialize_rows(self.cancel.as_ref());
            let result = table.serialize_field("rows", &rows);
            *self.failure.borrow_mut() = rows.take_failure();
            result?;
        } else {
            table.serialize_field("rows", &[] as &[()])?;
        }
        table.end()
    }
}
