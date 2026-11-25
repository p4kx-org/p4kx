// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;

use p4kx_export::catalog_queries::equipment::{
    AmmoBinding, AmmoContainer, WeaponActions, WeaponSourceSnapshot, WeaponSourceVisitSummary,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogRecordCoverage, CatalogRecordSelection, DatasetAvailability,
    ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, SourcedValue};

use super::equipment_input;
use crate::real_archive_catalog_table_parity::{
    DigestedInventory, DigestedTable, PhysicalSlot, TestResult,
    with_bound_measurement_from_producer,
};
use crate::real_archive_verification::print_line;

const TABLES: [&str; 3] = [
    "/configurations/weaponActions",
    "/configurations/ammoContainers",
    "/declarations/ammoBindings",
];

#[test]
#[ignore = "same Build: configure the installed Archive, completed Normal Catalog and exact weapon producer revision"]
fn installed_selected_weapon_sources_match_retained_complete_catalog() -> TestResult {
    let revision = env::var("P4KX_WEAPON_SOURCE_CATALOG_PRODUCER_REVISION")
        .map_err(|_| "configure the exact Weapon Source Catalog producer revision")?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let sdk = expected_tables(archive)?;
        let compared = measurement.verify_digested_tables(TABLES, &sdk.tables, &sdk.inventory)?;
        let [actions, containers, bindings] = compared.rows;
        let [direct_actions, direct_containers, direct_bindings] = sdk.direct;
        let [_, linked_containers, linked_bindings] =
            sdk.tables.each_ref().map(DigestedTable::dependency_count);
        print_line(&format!(
            "Weapon source retained parity: weaponActions={actions} ammoContainers={containers} ammoBindings={bindings} directWeaponActions={direct_actions} directAmmoContainers={direct_containers} directAmmoBindings={direct_bindings} linkedAmmoContainers={linked_containers} linkedAmmoBindings={linked_bindings} sourceUses={} physicalSources={}",
            compared.source_uses, compared.physical_sources,
        ));
        Ok(())
    })
}

#[derive(Default)]
struct ExpectedTables {
    tables: [DigestedTable; 3],
    inventory: DigestedInventory,
    direct: [usize; 3],
}

#[derive(Default)]
struct DirectScan {
    counts: [usize; 3],
    last_slot: Option<PhysicalSlot>,
}

impl DirectScan {
    fn finish(self, summary: &WeaponSourceVisitSummary) -> TestResult<[usize; 3]> {
        if !matches!(summary.availability(), DatasetAvailability::Available)
            || !complete(summary.coverage())
            || self.counts.contains(&0)
        {
            return Err(format!(
                "incomplete full weapon SDK scope: owners={} unreadableHeaders={} unreadableBodies={} incompleteFields={}",
                summary.owner_count(),
                summary.coverage().unreadable_headers.len(),
                summary.coverage().unreadable_bodies.len(),
                summary.coverage().incomplete_fields.len(),
            ).into());
        }
        Ok(self.counts)
    }

    fn inspect(&mut self, archive: &Archive, snapshot: &WeaponSourceSnapshot<'_>) -> TestResult {
        if snapshot.build() != archive.build()
            || !snapshot.requested_languages().is_empty()
            || !complete(snapshot.coverage())
        {
            return Err("a physical owner has incomplete or foreign weapon observations".into());
        }
        count_bindings(snapshot, &mut self.counts, &mut self.last_slot)
    }
}

#[test]
#[ignore = "real Archive: measure the full incremental route in a fresh process"]
fn installed_weapon_source_visit_cold() -> TestResult {
    equipment_input::with_archive(|archive| {
        let mut scan = DirectScan::default();
        let mut source_use_occurrences = 0;
        let mut physical_source_occurrences = 0;
        let session =
            equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
        let summary = session.visit_weapon_sources(
            CatalogRecordSelection::All,
            None,
            |_, snapshot| -> TestResult {
                scan.inspect(archive, &snapshot)?;
                let inventory = snapshot.source_inventory();
                source_use_occurrences += inventory.use_count();
                physical_source_occurrences += inventory.source_count();
                Ok(())
            },
        )?;
        let [actions, containers, bindings] = scan.finish(&summary)?;
        if source_use_occurrences == 0 || physical_source_occurrences == 0 {
            return Err("the incremental weapon SDK has no original source inventory".into());
        }
        print_line(&format!(
            "Weapon source visitor cold: owners={} weaponActions={actions} ammoContainers={containers} ammoBindings={bindings} sourceUseOccurrences={source_use_occurrences} physicalSourceOccurrences={physical_source_occurrences}",
            summary.owner_count(),
        ));
        Ok(())
    })
}

fn expected_tables(archive: &Archive) -> TestResult<ExpectedTables> {
    let session =
        equipment_input::full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
    let parsed = session.parsed(None)?;
    let mut expected = ExpectedTables::default();
    let mut scan = DirectScan::default();
    let mut links = Vec::new();
    let mut container_owners = HashSet::new();
    let summary = parsed.visit_weapon_sources(
        CatalogRecordSelection::All,
        None,
        |_, snapshot| -> TestResult {
            scan.inspect(archive, &snapshot)?;
            retain_direct(&snapshot, &mut expected, &mut container_owners, &mut links)
        },
    )?;
    expected.direct = scan.finish(&summary)?;
    for (weapon, target) in links {
        if !container_owners.contains(&target) {
            retain_linked(archive, &parsed, weapon, target, &mut expected)?;
        }
    }
    Ok(expected)
}

fn retain_direct(
    snapshot: &WeaponSourceSnapshot<'_>,
    expected: &mut ExpectedTables,
    container_owners: &mut HashSet<u32>,
    candidate_links: &mut Vec<(u32, u32)>,
) -> TestResult {
    let [actions, containers, bindings] = &mut expected.tables;
    let mut references = HashSet::new();
    for row in snapshot.rows() {
        for action in &row.weapon_actions {
            let owner = action.owner();
            let slot = physical_slot(owner.record_index(), owner.component_ordinal())?;
            actions.retain_direct(slot, action, &mut references)?;
        }
        for container in &row.ammo_containers {
            let owner = container.owner();
            let slot = physical_slot(owner.record_index(), owner.component_ordinal())?;
            container_owners.insert(slot.record_index);
            containers.retain_direct(slot, container, &mut references)?;
        }
        for binding in &row.ammo_bindings {
            let owner = binding.owner();
            let slot = physical_slot(owner.record_index(), owner.component_ordinal())?;
            bindings.retain_direct(slot, binding, &mut references)?;
        }
        if row.ammo_containers.is_empty()
            && let Some(AmmoBinding::WeaponMagazineSlots { value }) = row.ammo_bindings.first()
            && let Some(target) = value
                .primary_magazine()
                .value()
                .and_then(|reference| reference.resolved_index())
        {
            let weapon = value
                .owner()
                .record_index()
                .value()
                .copied()
                .ok_or("the original weapon Record index")?;
            candidate_links.push((weapon, target));
        }
    }
    if !references.is_empty() {
        expected
            .inventory
            .retain(snapshot.source_inventory(), references)?;
    }
    Ok(())
}

fn retain_linked(
    archive: &Archive,
    parsed: &ParsedCatalogQueries<'_>,
    weapon: u32,
    target: u32,
    expected: &mut ExpectedTables,
) -> TestResult {
    let [_, containers, bindings] = &mut expected.tables;
    parsed.with_weapon_magazine_dependencies(weapon, None, None, |snapshot| {
        if snapshot.build() != archive.build() || !complete(snapshot.coverage()) {
            return Err(format!("the primary magazine route has incomplete coverage for physical weapon Record {weapon}").into());
        }
        let mut references = HashSet::new();
        for route in snapshot.rows() {
            if !complete(&route.dependency_coverage) {
                return Err(format!("the linked magazine has incomplete coverage at physical Record {target}").into());
            }
            for dependency in &route.dependencies {
                let owner = dependency.container.owner();
                let slot = physical_slot(owner.record_index(), owner.component_ordinal())?;
                let binding = dependency.binding.owner();
                let binding_slot = physical_slot(binding.record_index(), binding.component_ordinal())?;
                if slot.record_index != target || slot != binding_slot || !matches!(&dependency.binding, AmmoBinding::ContainerSlots { .. }) {
                    return Err("the producer-only magazine rows lack their original physical slot pairing".into());
                }
                containers.retain_dependency(slot, &dependency.container, &mut references)?;
                bindings.retain_dependency(slot, &dependency.binding, &mut references)?;
            }
        }
        if !references.is_empty() {
            expected.inventory.retain(snapshot.source_inventory(), references)?;
        }
        Ok(())
    })?
}

fn count_bindings(
    snapshot: &WeaponSourceSnapshot<'_>,
    counts: &mut [usize; 3],
    last_slot: &mut Option<PhysicalSlot>,
) -> TestResult {
    for row in snapshot.rows() {
        let mut actions = row.weapon_actions.iter();
        let mut containers = row.ammo_containers.iter();
        for binding in &row.ammo_bindings {
            let owner = binding.owner();
            let slot = physical_slot(owner.record_index(), owner.component_ordinal())?;
            if last_slot.is_some_and(|last| last >= slot) {
                return Err("the standalone weapon bindings are unordered or duplicated".into());
            }
            *last_slot = Some(slot);
            let authored = match binding {
                AmmoBinding::WeaponMagazineSlots { .. } => {
                    counts[0] += 1;
                    actions.next().map(WeaponActions::owner)
                }
                AmmoBinding::ContainerSlots { .. } => {
                    counts[1] += 1;
                    containers.next().map(AmmoContainer::owner)
                }
                _ => None,
            }
            .ok_or("an original binding has no matching authored component")?;
            if physical_slot(authored.record_index(), authored.component_ordinal())? != slot {
                return Err(
                    "a binding and authored component refer to different physical slots".into(),
                );
            }
            counts[2] += 1;
        }
        if actions.next().is_some() || containers.next().is_some() {
            return Err("an authored component lacks its original binding".into());
        }
    }
    Ok(())
}

const fn complete(coverage: &CatalogRecordCoverage) -> bool {
    coverage.unreadable_headers.is_empty()
        && coverage.unreadable_bodies.is_empty()
        && coverage.incomplete_fields.is_empty()
}

fn physical_slot(
    record: &SourcedValue<u32>,
    ordinal: &SourcedValue<u32>,
) -> TestResult<PhysicalSlot> {
    match (record.value(), ordinal.value()) {
        (Some(&record), Some(&ordinal)) => Ok(PhysicalSlot {
            record_index: record,
            component_ordinal: ordinal,
        }),
        _ => Err("the row lacks a physical Record index or Components slot".into()),
    }
}
