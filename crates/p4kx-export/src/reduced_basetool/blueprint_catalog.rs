// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::blueprint_taxonomy;
use super::blueprints::{self, Blueprints};
use super::dismantle_processes;
use super::record_targets::RecordTargets;
use super::selected_texts::SelectedTexts;
use crate::catalog_queries::ParsedCatalogQueries;
use crate::catalog_rows::staged::StagedCatalogTable;

pub(super) struct BlueprintCatalog {
    pub(super) definitions: StagedCatalogTable,
    pub(super) presentations: StagedCatalogTable,
    pub(super) tiers: StagedCatalogTable,
    pub(super) categories: StagedCatalogTable,
    pub(super) pools: StagedCatalogTable,
    pub(super) pool_weights: StagedCatalogTable,
    pub(super) pool_targets: RecordTargets,
    pub(super) dismantle_processes: StagedCatalogTable,
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    texts: &mut SelectedTexts,
) -> Result<BlueprintCatalog, ReducedError> {
    let processes = dismantle_processes::collect(parsed, scratch, cancel)?;
    let Blueprints {
        definitions,
        presentations,
        tiers,
        targets,
    } = blueprints::collect(parsed, scratch, cancel, texts, &processes)?;
    let categories = blueprint_taxonomy::collect_categories(parsed, scratch, cancel)?;
    let pools = blueprint_taxonomy::collect_pools(parsed, scratch, cancel, &targets)?;
    Ok(BlueprintCatalog {
        definitions,
        presentations,
        tiers,
        categories,
        pools: pools.definitions,
        pool_weights: pools.weights,
        pool_targets: pools.targets,
        dismantle_processes: processes.definitions,
    })
}
