// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use crate::catalog_queries::equipment::ComponentOwner;
use crate::guid_text::CatalogGuid;

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
