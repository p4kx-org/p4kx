// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

use super::spool::{RowSpool, RowSpoolError, RowSpoolWriter, checkpoint};
use crate::catalog_table::DatasetAvailability;

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
