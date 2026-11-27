// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{Archive, CancelToken};

use super::selected_tables::SelectedTables;
use super::selected_texts::SelectedTexts;
use super::{
    ReducedError, cargo_globals, commodities, crafting_global, resource_components,
    resource_groups, resource_types, shops,
};
use crate::catalog_queries::ParsedCatalogQueries;

pub(super) fn stage(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    tables: &mut SelectedTables,
    texts: &mut SelectedTexts,
) -> Result<(), ReducedError> {
    let cargo = cargo_globals::collect(parsed, scratch, cancel)?;
    tables.table(cargo_globals::LOADING, cargo.loading)?;
    tables.table(cargo_globals::COMMODITY, cargo.commodity)?;

    shops::collect(archive, scratch, cancel)?.add_to(tables)?;

    let commodities = commodities::collect(parsed, scratch, cancel, texts)?;
    tables.table(commodities::DEFINITIONS, commodities.definitions)?;
    tables.table(commodities::PRESENTATIONS, commodities.presentations)?;

    let resources = resource_types::collect(parsed, scratch, cancel, texts)?;
    tables.table(resource_types::PATH, resources.definitions)?;
    tables.table(resource_groups::DEFINITIONS, resources.groups.definitions)?;
    tables.table(resource_groups::MEMBERS, resources.groups.members)?;
    tables.table(resource_groups::PARENTS, resources.groups.parents)?;
    tables.table(crafting_global::PATH, resources.crafting_global)?;
    tables.table(resource_components::PATH, resources.components)
}
