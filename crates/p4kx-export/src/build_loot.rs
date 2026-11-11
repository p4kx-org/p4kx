// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken};

use crate::CatalogError;
use crate::catalog_read::original::EntryCapture;
use crate::collected::Collected;
use crate::crafting_warning::CraftingWarning;
use crate::harvest_locations::Objects;
use crate::loot_containers::{Located, locate};
use crate::loot_walk::MAX_CONTAINERS;
use crate::starmap_containers::Placements;

/// Finds the containers below the system containers that are placed with a multi config and
/// stores them as the loot containers of `collected`. A container that cannot be read, or that an
/// entity places and the Archive lacks, is warned about, as a config it carries may be missing or
/// its counts may be absent; so is an entity file that names several containers or configs.
///
/// # Errors
///
/// Returns [`CatalogError::Cancelled`] once `cancel` is cancelled.
pub(crate) fn add_loot_containers(
    archive: &Archive,
    placements: &Placements,
    collected: &mut Collected<'_>,
    cancel: Option<&CancelToken>,
    capture: &EntryCapture<'_>,
) -> Result<(), CatalogError> {
    crate::entry_source::dependencies(
        &placements.ledger,
        &mut collected.ledger,
        crate::Kind::LootContainers,
    );
    let located = locate(
        archive,
        &placements.opened,
        &Objects::new(&collected.starmap_objects),
        cancel,
        capture,
    )?;
    if cancel.is_some_and(CancelToken::is_cancelled) {
        return Err(CatalogError::Cancelled);
    }
    let Located {
        ledger,
        containers,
        unreadable,
        beyond_limit,
        several,
    } = located;
    collected.ledger.merge(ledger);
    let mut details: Vec<String> = unreadable.iter().map(ToString::to_string).collect();
    if beyond_limit > 0 {
        details.push(format!(
            "{beyond_limit} containers beyond the limit of {MAX_CONTAINERS} are not read"
        ));
    }
    if !details.is_empty() {
        collected
            .warnings
            .push(CraftingWarning::UnreadableLootContainers {
                count: details.len(),
                details,
            });
    }
    if !several.is_empty() {
        collected
            .warnings
            .push(CraftingWarning::AmbiguousLootEntityFiles {
                count: several.len(),
                details: several,
            });
    }
    collected.loot_containers = containers;
    Ok(())
}
