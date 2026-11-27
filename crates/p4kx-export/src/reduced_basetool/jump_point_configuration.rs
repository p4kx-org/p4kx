// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::record_link::RecordLink;
use crate::catalog_queries::equipment::jump_points::JumpPointConfiguration;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

pub(super) const PATH: &str = "configurations.jumpPointConfigurations";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_jump_point_configurations(CatalogRecordSelection::All, cancel, |input| {
        completeness::inventory(PATH, input.availability(), input.coverage())?;
        let mut staged = StagedCatalogRows::new(scratch, cancel);
        for configuration in input.rows() {
            staged.append(&Row::read(configuration)?)?;
        }
        let table = staged.finish(input.availability())?;
        completeness::rows(PATH, input.rows().len(), table.len())?;
        Ok(table)
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Row<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    largest_ship_size: Cell<RecordLink<'read>>,
}

impl<'read> Row<'read> {
    fn read(value: &'read JumpPointConfiguration<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(value.owner(), PATH)?;
        Ok(Self {
            owner: value.owner().into(),
            largest_ship_size: completeness::cell(PATH, value.largest_ship_size())?
                .try_map(|reference| RecordLink::from_source(PATH, reference))?,
        })
    }
}
