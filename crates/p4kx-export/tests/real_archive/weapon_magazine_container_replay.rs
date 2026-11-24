// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::equipment::{AmmoBinding, AmmoContainer, EquipmentSnapshot};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, ParsedCatalogQueries,
};
use p4kx_extraction::{Archive, DataForgeEntry, RecordTree, SourceInventory, Value as Raw};
use serde_json::Value;

use super::{CONTAINER, Witness, check_outcome, find, original};
use crate::real_archive_catalog_table_parity::{Origins, TestResult, check_selected_row, wire_row};

pub(crate) const PATHS: [&str; 2] = [
    "/configurations/ammoContainers/rows",
    "/declarations/ammoBindings/rows",
];

pub(crate) struct Rows {
    container: Value,
    binding: Value,
}

pub(crate) fn read(archive: &Archive, origins: &mut Origins) -> TestResult<[Rows; 2]> {
    let entry = DataForgeEntry::read(archive)?;
    let forge = entry.open()?;
    let [own, linked] = find(&forge)?;
    let target = linked
        .magazine
        .as_ref()
        .ok_or("the original linked magazine")?;
    if target.record_index() == own.owner.record_index() {
        return Err("the retained witness requires two distinct physical magazine owners".into());
    }
    let session = CatalogQuerySession::from_entry(
        archive,
        &entry,
        CatalogLanguageSelection::Selected(&[]),
        None,
    )?;
    let parsed = session.parsed(None)?;
    let own = selected_rows(&parsed, archive, &own.owner, own.slot, origins)?;
    let linked = match dependency_rows(&parsed, archive, &linked, target, origins)? {
        Some(dependency) => dependency,
        None => selected_rows(&parsed, archive, target, linked.slot, origins)?,
    };
    Ok([own, linked])
}

fn selected_rows(
    parsed: &ParsedCatalogQueries<'_>,
    archive: &Archive,
    owner: &RecordTree<'_>,
    ordinal: usize,
    origins: &mut Origins,
) -> TestResult<Rows> {
    parsed.with_weapon_sources(owner.record_index(), None, |snapshot| {
        check_scope(archive, &snapshot)?;
        let [row] = snapshot.rows() else {
            return Err("one physically selected Item with authored container inputs".into());
        };
        let [container] = row.ammo_containers.as_slice() else {
            return Err("one physical container on the independently nominated Item".into());
        };
        let mut bindings = row
            .ammo_bindings
            .iter()
            .filter(|binding| matches!(binding, AmmoBinding::ContainerSlots { .. }));
        let (Some(binding), None) = (bindings.next(), bindings.next()) else {
            return Err("one original container binding on the nominated Item".into());
        };
        compare_and_capture(
            owner,
            ordinal,
            container,
            binding,
            origins,
            snapshot.source_inventory(),
        )
    })?
}

fn dependency_rows(
    parsed: &ParsedCatalogQueries<'_>,
    archive: &Archive,
    linked: &Witness<'_>,
    target: &RecordTree<'_>,
    origins: &mut Origins,
) -> TestResult<Option<Rows>> {
    parsed.with_weapon_magazine_dependencies(
        linked.owner.record_index(),
        None,
        None,
        |snapshot| {
            check_scope(archive, &snapshot)?;
            let [row] = snapshot.rows() else {
                return Err("one original linked magazine route".into());
            };
            check_outcome(row.resolution.outcome().value(), linked)?;
            if !row.dependency_coverage.unreadable_headers.is_empty()
                || !row.dependency_coverage.unreadable_bodies.is_empty()
                || !row.dependency_coverage.incomplete_fields.is_empty()
            {
                return Err(
                    "the linked magazine dependency must have complete source coverage".into(),
                );
            }
            let dependency = match row.dependencies.as_slice() {
                [] => return Ok(None),
                [dependency] => dependency,
                _ => {
                    return Err(
                        "the nominated linked route reaches more than one magazine slot".into(),
                    );
                }
            };
            compare_and_capture(
                target,
                linked.slot,
                &dependency.container,
                &dependency.binding,
                origins,
                snapshot.source_inventory(),
            )
            .map(Some)
        },
    )?
}

fn check_scope<Row, Dependencies>(
    archive: &Archive,
    snapshot: &EquipmentSnapshot<'_, Row, Dependencies>,
) -> TestResult {
    let coverage = snapshot.coverage();
    if snapshot.build() != archive.build()
        || !coverage.unreadable_headers.is_empty()
        || !coverage.unreadable_bodies.is_empty()
        || !coverage.incomplete_fields.is_empty()
        || !snapshot.requested_languages().is_empty()
    {
        return Err("the installed container scope must match the Build and preserve complete physical inputs".into());
    }
    Ok(())
}

fn compare_and_capture(
    owner: &RecordTree<'_>,
    ordinal: usize,
    container: &AmmoContainer<'_>,
    binding: &AmmoBinding<'_>,
    origins: &mut Origins,
    inventory: &SourceInventory,
) -> TestResult<Rows> {
    let (slot, raw) =
        original::unique(owner, CONTAINER).ok_or("one original container component")?;
    let capacity = original::structure(raw).and_then(|source| source.field("maxAmmoCount"));
    let Some(Raw::Int32(capacity)) = capacity else {
        return Err("the original candidate must author a readable signed capacity".into());
    };
    if slot != ordinal
        || container.max_ammo_count().value() != Some(capacity)
        || !container.max_ammo_count().origin().source_set_complete()
        || !matches!(binding, AmmoBinding::ContainerSlots { .. })
    {
        return Err("the SDK must name the original container and signed capacity".into());
    }
    let ordinal = u32::try_from(ordinal)?;
    for source in [container.owner(), binding.owner()] {
        if source.record_index().value() != Some(&owner.record_index())
            || source.component_ordinal().value() != Some(&ordinal)
        {
            return Err(
                "the configuration and binding must share the original physical slot".into(),
            );
        }
    }
    let rows = Rows {
        container: wire_row(container)?,
        binding: wire_row(binding)?,
    };
    origins.extend(inventory, std::slice::from_ref(&rows.container))?;
    origins.extend(inventory, std::slice::from_ref(&rows.binding))?;
    Ok(rows)
}

pub(crate) fn verify(catalog: &Value, rows: &[Rows; 2]) -> TestResult {
    let [containers, bindings] = PATHS;
    for row in rows {
        check_selected_row(catalog, containers, &row.container, |candidate| {
            same_slot(candidate, &row.container)
        })?;
        check_selected_row(catalog, bindings, &row.binding, |candidate| {
            candidate.get("type") == row.binding.get("type")
                && matches!((candidate.get("value"), row.binding.get("value")),
                    (Some(actual), Some(expected)) if same_slot(actual, expected))
        })?;
    }
    Ok(())
}

fn same_slot(actual: &Value, expected: &Value) -> bool {
    ["/ownerRecordIndex/value", "/componentOrdinal/value"]
        .into_iter()
        .all(|path| actual.pointer(path) == expected.pointer(path))
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_magazine_container_cold_query() -> TestResult {
    let path = p4kx_testkit::real_archive_path()?;
    let key = p4kx_testkit::real_archive_key()?;
    let archive = Archive::open_with_key(path, Some(key))?;
    let mut origins = Origins::default();
    let rows = read(&archive, &mut origins)?;
    crate::real_archive_verification::print_line(&format!(
        "Scoped Ammo container query: {} configurations and {} original bindings",
        rows.len(),
        rows.len()
    ));
    Ok(())
}
