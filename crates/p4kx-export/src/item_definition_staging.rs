// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, DataForge};

use super::{ItemDefinition, WeaponAttachment};
use crate::catalog_rows::staged::{IndexedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::guid_text::GuidText;
use crate::{CatalogError, Kind};

type DefinitionKey = (GuidText, u32);

/// Stages complete original item rows and keeps the small weapon input for later joins.
pub(crate) struct ItemDefinitionStaging<'stage, 'source> {
    rows: IndexedCatalogRows<'stage, DefinitionKey>,
    weapons: Vec<(DefinitionKey, WeaponAttachment<'source>)>,
}

impl<'stage, 'source> ItemDefinitionStaging<'stage, 'source> {
    pub(crate) const fn new(directory: &'stage Path, cancel: Option<&'stage CancelToken>) -> Self {
        Self {
            rows: IndexedCatalogRows::new(directory, cancel),
            weapons: Vec::new(),
        }
    }

    pub(crate) fn append(&mut self, row: &ItemDefinition<'source>) -> Result<(), CatalogError> {
        let key = row.order_key()?;
        self.rows.append(key, row)?;
        self.weapons
            .extend(row.weapon_attachments().map(|attachment| (key, attachment)));
        Ok(())
    }

    pub(crate) fn finish(
        mut self,
        forge: &DataForge<'_>,
    ) -> Result<StagedItemDefinitions<'source>, CatalogError> {
        let table = self
            .rows
            .finish(forge, &[Kind::Items.records().type_name])?;
        self.weapons.sort_by_key(|(key, _)| *key);
        Ok(StagedItemDefinitions {
            table,
            weapons: self.weapons.into_iter().map(|(_, weapon)| weapon).collect(),
        })
    }
}

pub(crate) struct StagedItemDefinitions<'source> {
    pub(crate) table: StagedCatalogTable,
    pub(crate) weapons: Vec<WeaponAttachment<'source>>,
}

impl Default for StagedItemDefinitions<'_> {
    fn default() -> Self {
        Self {
            table: StagedCatalogTable::empty(DatasetAvailability::Absent),
            weapons: Vec::new(),
        }
    }
}
