// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::ser::SerializeSeq;
use serde::{Serialize, Serializer};

use super::join::ShipLoadoutJoin;
use crate::catalog_rows::spool::{RowSpool, RowSpoolError, RowSpoolWriter, checkpoint};
use crate::contract_loadout::{EntityKind, Loadout};

/// Canonical Loadout rows and the typed inputs for Ship placement joins.
#[derive(Debug, Default)]
pub(crate) struct StagedLoadouts {
    ships: Vec<ShipLoadoutJoin>,
    rows: Option<RowSpool>,
    cancel: Option<CancelToken>,
    failure: RefCell<Option<RowSpoolError>>,
}

pub(super) struct StagedLoadoutWriter<'run> {
    ships: Vec<ShipLoadoutJoin>,
    rows: Option<RowSpoolWriter>,
    scratch: &'run Path,
    cancel: Option<&'run CancelToken>,
}

impl<'run> StagedLoadoutWriter<'run> {
    pub(super) const fn new(scratch: &'run Path, cancel: Option<&'run CancelToken>) -> Self {
        Self {
            ships: Vec::new(),
            rows: None,
            scratch,
            cancel,
        }
    }

    pub(super) fn append(&mut self, loadout: Loadout) -> Result<(), RowSpoolError> {
        checkpoint(self.cancel)?;
        let writer = match &mut self.rows {
            Some(writer) => writer,
            None => self.rows.insert(RowSpoolWriter::new(self.scratch)?),
        };
        writer.append(&loadout, self.cancel)?;
        if loadout.entity_kind() == EntityKind::Ship {
            self.ships
                .push(ShipLoadoutJoin::retain(loadout, self.cancel)?);
        }
        Ok(())
    }

    pub(super) fn finish(self) -> Result<StagedLoadouts, RowSpoolError> {
        checkpoint(self.cancel)?;
        let rows = self
            .rows
            .map(|writer| writer.finish(self.cancel))
            .transpose()?;
        Ok(StagedLoadouts {
            ships: self.ships,
            rows,
            cancel: self.cancel.cloned(),
            failure: RefCell::default(),
        })
    }
}

impl StagedLoadouts {
    #[cfg(test)]
    pub(crate) fn stage(
        rows: Vec<Loadout>,
        scratch: &Path,
        cancel: Option<&CancelToken>,
    ) -> Result<Self, RowSpoolError> {
        let mut writer = StagedLoadoutWriter::new(scratch, cancel);
        for row in rows {
            writer.append(row)?;
        }
        writer.finish()
    }

    pub(crate) const fn len(&self) -> usize {
        match &self.rows {
            Some(rows) => rows.rows(),
            None => 0,
        }
    }

    pub(crate) fn ships(&self) -> &[ShipLoadoutJoin] {
        &self.ships
    }

    pub(crate) fn release_ship_joins(&mut self) {
        self.ships = Vec::new();
    }

    pub(crate) fn stored_bytes(&self) -> u64 {
        self.rows.as_ref().map_or(0, RowSpool::stored_bytes)
    }

    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.failure.take()
    }
}

impl Serialize for StagedLoadouts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let Some(spool) = &self.rows else {
            return serializer.serialize_seq(Some(0))?.end();
        };
        let rows = spool.serialize_rows(self.cancel.as_ref());
        let result = rows.serialize(serializer);
        *self.failure.borrow_mut() = rows.take_failure();
        result
    }
}

#[cfg(test)]
#[path = "staged_loadouts_tests.rs"]
mod tests;
