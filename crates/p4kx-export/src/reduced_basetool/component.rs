// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use crate::catalog_queries::equipment::{ComponentOwner, EquipmentSnapshot};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::guid_text::CatalogGuid;

pub(super) fn stage_selected<'read, Source, Reduced: Serialize>(
    path: &'static str,
    snapshot: &'read EquipmentSnapshot<'_, Source>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    mut project: impl FnMut(&'read Source) -> Result<Reduced, ReducedError>,
) -> Result<StagedCatalogTable, ReducedError> {
    let availability = snapshot.availability();
    completeness::inventory(path, availability, snapshot.coverage())?;
    let mut output = StagedCatalogRows::new(scratch, cancel);
    for row in snapshot.rows() {
        output.append(&project(row)?)?;
    }
    let table = output.finish(availability)?;
    completeness::rows(path, snapshot.rows().len(), table.len())?;
    Ok(table)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct ComponentKey {
    pub(super) record: u32,
    pub(super) guid: CatalogGuid,
    pub(super) ordinal: u32,
}

impl ComponentKey {
    pub(super) fn read(owner: &ComponentOwner, path: &'static str) -> Result<Self, ReducedError> {
        let missing = || ReducedError::IncompleteScope(path);
        Ok(Self {
            record: owner.record_index().value().copied().ok_or_else(missing)?,
            guid: owner.guid().value().copied().ok_or_else(missing)?,
            ordinal: owner
                .component_ordinal()
                .value()
                .copied()
                .ok_or_else(missing)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Component<'source> {
    owner_guid: Cell<&'source CatalogGuid>,
    owner_record_index: Cell<&'source u32>,
    component_ordinal: Cell<&'source u32>,
}

impl<'source> From<&'source ComponentOwner> for Component<'source> {
    fn from(owner: &'source ComponentOwner) -> Self {
        Self {
            owner_guid: owner.guid().into(),
            owner_record_index: owner.record_index().into(),
            component_ordinal: owner.component_ordinal().into(),
        }
    }
}
