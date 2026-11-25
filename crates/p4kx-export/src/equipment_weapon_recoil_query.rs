// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForge, DataForgeEntry};

use super::EquipmentSnapshot;
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::unlocalized_record_query::plan::Plan;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_query_session::{CatalogQuerySource, checkpoint};
use crate::catalog_v3::catalog_weapon_actions::RECOIL_DEFINITION;
use crate::catalog_v3::weapon_action_recoil_definitions;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::weapon_action_recoil_definitions::WeaponActionRecoilDefinition;

/// Selected canonical recoil definitions and their original source inventory.
pub type WeaponRecoilSnapshot<'source> =
    EquipmentSnapshot<'source, WeaponActionRecoilDefinition<'source>>;

/// Supplies canonical recoil definitions for the selected physical Records.
/// Join action links through their physical target Record index.
///
/// # Errors
/// Returns input, source-evidence, Archive-change or cancellation errors.
pub fn with_weapon_recoil_definitions<T>(
    archive: &Archive,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponRecoilSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), cancel)?
        .with_weapon_recoil_definitions(selection, cancel, inspect)
}

/// Supplies selected recoil definitions using an already decoded original Archive Entry.
///
/// # Errors
/// Returns query errors or a source-origin mismatch for a foreign Entry.
pub fn with_weapon_recoil_definitions_from<T>(
    archive: &Archive,
    entry: &DataForgeEntry,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponRecoilSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    CatalogQuerySession::from_entry(
        archive,
        entry,
        CatalogLanguageSelection::Selected(&[]),
        cancel,
    )?
    .with_weapon_recoil_definitions(selection, cancel, inspect)
}

impl CatalogQuerySession<'_> {
    /// Supplies selected recoil definitions through this Archive session.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_recoil_definitions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponRecoilSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.parsed(cancel)?
            .with_weapon_recoil_definitions(selection, cancel, inspect)
    }
}

impl ParsedCatalogQueries<'_> {
    /// Supplies selected recoil definitions through the parsed DataForge view.
    ///
    /// # Errors
    /// Returns canonical query, Archive-change or cancellation errors.
    pub fn with_weapon_recoil_definitions<T>(
        &self,
        selection: CatalogRecordSelection,
        cancel: Option<&CancelToken>,
        inspect: impl FnOnce(WeaponRecoilSnapshot<'_>) -> T,
    ) -> Result<T, CatalogError> {
        self.project(cancel, |archive, entry, forge| {
            with_parsed(archive, entry, forge, selection, cancel, inspect)
        })
    }
}

fn with_parsed<T>(
    archive: &Archive,
    entry: CatalogQuerySource<'_>,
    forge: &DataForge<'_>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
    inspect: impl FnOnce(WeaponRecoilSnapshot<'_>) -> T,
) -> Result<T, CatalogError> {
    let mut plan = Plan::read_type(forge, RECOIL_DEFINITION, selection, cancel)?;
    let origins = entry.records(archive)?;
    let input = OriginalCatalogInput::new(forge, &origins);
    let mut rows = Vec::with_capacity(plan.owners.len());
    for owner in plan.owners {
        checkpoint(cancel)?;
        let admission = RecordObservation::capture(&input, &owner.record)?;
        let read = weapon_action_recoil_definitions::read_selected(
            &owner.tree,
            &input,
            &admission,
            cancel,
        )?;
        if !read.selection_complete {
            plan.coverage.incomplete_fields.push(owner.record.index());
        }
        rows.push(read.value);
    }
    let table = weapon_action_recoil_definitions::table(forge, rows);
    let source_inventory = origins.uses.finish_with(|| checkpoint(cancel))?;
    checkpoint(cancel)?;
    Ok(inspect(EquipmentSnapshot {
        build: archive.build().cloned(),
        availability: table.availability,
        rows: table.rows,
        public_names: Vec::new(),
        game_texts: Vec::new(),
        requested_languages: Vec::new(),
        available_languages: Vec::new(),
        localization_notes: Vec::new(),
        source_inventory,
        coverage: plan.coverage,
        dependency_coverage: (),
    }))
}
