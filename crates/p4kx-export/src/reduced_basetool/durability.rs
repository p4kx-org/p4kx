// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::number::Number;
use crate::catalog_queries::equipment::durability::Durability;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

pub(super) const PATH: &str = "configurations.durability";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_durability(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability(), snapshot.coverage())?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        for item in snapshot.rows() {
            rows.append(&Health::read(item)?)?;
        }
        let table = rows.finish(snapshot.availability())?;
        completeness::rows(PATH, snapshot.rows().len(), table.len())?;
        Ok(table)
    })?
}

#[derive(Serialize)]
struct Health<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    health: Cell<Number<'read>>,
}

impl<'read> Health<'read> {
    fn read(source: &'read Durability<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(source.owner(), PATH)?;
        Ok(Self {
            owner: source.owner().into(),
            health: completeness::cell(PATH, source.health())?.map(Number::from),
        })
    }
}
