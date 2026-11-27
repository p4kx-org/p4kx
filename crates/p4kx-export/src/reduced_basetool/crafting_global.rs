// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::record_identity::RecordIdentity;
use super::record_link::RecordLink;
use super::record_targets::RecordTargets;
use crate::Kind;
use crate::catalog_queries::crafting::CraftingGlobalConfiguration;
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};

pub(super) const PATH: &str = "configurations.craftingGlobal";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    resource_types: &RecordTargets,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_crafting_global_params(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability(), snapshot.coverage())?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        for original in snapshot.rows() {
            rows.append(&Configuration::read(original, resource_types)?)?;
        }
        let table = rows.finish(snapshot.availability())?;
        completeness::rows(PATH, snapshot.rows().len(), table.len())?;
        Ok(table)
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Configuration<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    dismantle_blacklist_resources: Cell<Vec<Cell<RecordLink<'read>>>>,
}

impl<'read> Configuration<'read> {
    fn read(
        original: &'read CraftingGlobalConfiguration<'_>,
        resource_types: &RecordTargets,
    ) -> Result<Self, ReducedError> {
        let header = original.source();
        completeness::value(PATH, &header.guid)?;
        completeness::value(PATH, header.record_index())?;
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        Ok(Self {
            identity: header.into(),
            dismantle_blacklist_resources: completeness::list(
                PATH,
                original.dismantle_blacklist_resources(),
                |reference| {
                    RecordLink::read_source(PATH, reference, resource_types, |source_type| {
                        source_type == Kind::ResourceTypes.records().type_name
                    })
                },
            )?,
        })
    }
}
