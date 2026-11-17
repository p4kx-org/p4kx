// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::ser::{Error as _, SerializeSeq};
use serde::{Serialize, Serializer};

use crate::catalog_rows::spool::{RowSpool, RowSpoolError, RowSpoolWriter, checkpoint};
use crate::contract_loadout::{EntityKind, Loadout};

/// One ship loadout at its original position in the complete catalog.
#[derive(Debug)]
pub(crate) struct PositionedShipLoadout {
    pub(crate) ordinal: usize,
    pub(crate) loadout: Loadout,
}

/// Item rows are spooled while ship rows remain available for placement joins.
#[derive(Debug, Default)]
pub(crate) struct StagedLoadouts {
    ships: Vec<PositionedShipLoadout>,
    items: Option<RowSpool>,
    len: usize,
    cancel: Option<CancelToken>,
    failure: RefCell<Option<RowSpoolError>>,
}

pub(super) struct StagedLoadoutWriter<'run> {
    ships: Vec<PositionedShipLoadout>,
    items: Option<RowSpoolWriter>,
    len: usize,
    scratch: &'run Path,
    cancel: Option<&'run CancelToken>,
}

impl<'run> StagedLoadoutWriter<'run> {
    pub(super) const fn new(scratch: &'run Path, cancel: Option<&'run CancelToken>) -> Self {
        Self {
            ships: Vec::new(),
            items: None,
            len: 0,
            scratch,
            cancel,
        }
    }

    pub(super) fn append(&mut self, loadout: Loadout) -> Result<(), RowSpoolError> {
        checkpoint(self.cancel)?;
        match loadout.entity_kind {
            EntityKind::Ship => self.ships.push(PositionedShipLoadout {
                ordinal: self.len,
                loadout,
            }),
            EntityKind::Item => {
                let writer = match &mut self.items {
                    Some(writer) => writer,
                    None => self.items.insert(RowSpoolWriter::new(self.scratch)?),
                };
                writer.append(&loadout, self.cancel)?;
            }
        }
        self.len += 1;
        Ok(())
    }

    pub(super) fn finish(self) -> Result<StagedLoadouts, RowSpoolError> {
        checkpoint(self.cancel)?;
        let items = self
            .items
            .map(|writer| writer.finish(self.cancel))
            .transpose()?;
        Ok(StagedLoadouts {
            ships: self.ships,
            items,
            len: self.len,
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
        self.len
    }

    pub(crate) fn ships(&self) -> &[PositionedShipLoadout] {
        &self.ships
    }

    pub(crate) fn stored_bytes(&self) -> u64 {
        self.items.as_ref().map_or(0, RowSpool::stored_bytes)
    }

    pub(crate) fn take_failure(&self) -> Option<RowSpoolError> {
        self.failure.take()
    }
}

impl Serialize for StagedLoadouts {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        *self.failure.borrow_mut() = None;
        let mut written = 0;
        let mut ships = self.ships.iter().peekable();
        let mut sequence = serializer.serialize_seq(Some(self.len))?;
        let mut emit_ships = |sequence: &mut S::SerializeSeq, written: &mut usize| {
            while ships.peek().is_some_and(|ship| ship.ordinal == *written) {
                let ship = ships
                    .next()
                    .ok_or_else(|| S::Error::custom("missing ship row"))?;
                sequence.serialize_element(&ship.loadout)?;
                *written += 1;
            }
            Ok::<_, S::Error>(())
        };
        if let Some(items) = &self.items {
            let rows = items.serialize_rows(self.cancel.as_ref());
            let result = rows.visit(|item| {
                emit_ships(&mut sequence, &mut written)?;
                sequence.serialize_element(item)?;
                written += 1;
                Ok::<_, S::Error>(())
            });
            *self.failure.borrow_mut() = rows.take_failure();
            result?;
        }
        emit_ships(&mut sequence, &mut written)?;
        if written != self.len {
            return Err(S::Error::custom("loadout row count changed"));
        }
        sequence.end()
    }
}

#[cfg(test)]
#[path = "staged_loadouts_tests.rs"]
mod tests;
