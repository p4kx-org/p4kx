// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;

use super::ReducedError;
use super::ammo_definitions;
use super::completeness;
use super::component::ComponentKey;
use super::record_targets::RecordTargets;
use super::selected_tables::SelectedTables;
use super::weapon_actions;
use super::weapon_combat_bases;
use super::weapon_projectile_ammo_selections::{self, Targets};
use crate::catalog_queries::CatalogRecordSelection;
use crate::catalog_queries::equipment::CombinedWeaponOwner;
use crate::catalog_query_session::ParsedCatalogQueries;
use crate::catalog_rows::staged::StagedCatalogRows;

pub(super) fn stage(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
    items: &RecordTargets,
    tables: &mut SelectedTables,
) -> Result<(), ReducedError> {
    let (ammunition, ammunition_targets) = ammo_definitions::collect(parsed, scratch, cancel)?;
    let targets = Targets {
        items,
        ammunition: &ammunition_targets,
    };
    let mut rows = Weapons {
        actions: StagedCatalogRows::new(scratch, cancel),
        combat: StagedCatalogRows::new(scratch, cancel),
        routes: StagedCatalogRows::new(scratch, cancel),
    };
    let summary = parsed.visit_combined_weapons(
        CatalogRecordSelection::All,
        cancel,
        |record, snapshot| -> Result<(), ReducedError> {
            for source in snapshot.rows() {
                completeness::inventory(
                    weapon_projectile_ammo_selections::PATH,
                    snapshot.availability(),
                    &source.combat.projectile_ammo.dependency_coverage,
                )?;
                rows.append(record, source, &targets, cancel)?;
            }
            Ok(())
        },
    )?;
    let availability = summary.availability();
    completeness::inventory(weapon_actions::PATH, availability, summary.coverage())?;
    tables.table(ammo_definitions::PATH, ammunition)?;
    tables.table(weapon_actions::PATH, rows.actions.finish(availability)?)?;
    tables.table(weapon_combat_bases::PATH, rows.combat.finish(availability)?)?;
    tables.table(
        weapon_projectile_ammo_selections::PATH,
        rows.routes.finish(availability)?,
    )
}

struct Weapons<'scratch> {
    actions: StagedCatalogRows<'scratch>,
    combat: StagedCatalogRows<'scratch>,
    routes: StagedCatalogRows<'scratch>,
}

impl Weapons<'_> {
    fn append(
        &mut self,
        record: u32,
        source: &CombinedWeaponOwner<'_>,
        targets: &Targets<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<(), ReducedError> {
        for action in &source.actions {
            let owner = ComponentKey::read(action.owner(), weapon_actions::PATH)?;
            targets
                .items
                .verify(weapon_actions::PATH, record, owner.guid.get())?;
            weapon_actions::append(&mut self.actions, record, action, cancel)?;
        }
        weapon_combat_bases::append(
            &mut self.combat,
            record,
            &source.combat.combat_bases,
            targets.items,
            targets.ammunition,
            cancel,
        )?;
        weapon_projectile_ammo_selections::append(
            &mut self.routes,
            &source.combat.projectile_ammo.selections,
            targets,
        )
    }
}
