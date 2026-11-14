// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::time::UNIX_EPOCH;

use p4kx_export::{
    CatalogError, CatalogOptions, CatalogPublicationMode, EntryOperation, ExportDiagnosticCode,
    ExportDiagnosticSeverity, Kind, build_catalog_from,
};

use p4kx_extraction::{Archive, ArchiveWatch, DataForge, DataForgeEntry, Filter, PathIndex};
use p4kx_testkit::{real_archive_key, real_archive_path};
use serde_json::Value;

#[path = "real_archive_facility_placement_comparison.rs"]
mod comparison;
#[path = "real_archive_facility_placement_inventory.rs"]
mod inventory;
#[path = "real_archive_facility_placement_sources.rs"]
mod source;

type Result<T = ()> = std::result::Result<T, Box<dyn std::error::Error>>;

const SHIP_SAMPLE: usize = 180;
const ROOM_SAMPLE: usize = 12;
const MAX_SELECTED_RECORDS: usize = 1_000;
const SMALL_ROOM_COUNT: usize = 3;
const FILTER_CANDIDATES: usize = 8;

#[derive(Clone, Copy)]
enum RoomCase {
    Small,
    Multi,
}

impl RoomCase {
    const fn label(self) -> &'static str {
        match self {
            Self::Small => "small direct",
            Self::Multi => "multi-room",
        }
    }
}

struct Case<'ship> {
    ship: &'ship inventory::Ship,
    component: usize,
    room: usize,
    witness: source::Witness,
    filter: Filter,
    selected_records: usize,
    nested_coverage: bool,
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_small_ship_facilities_match_original_placements() -> Result {
    run(RoomCase::Small)
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn installed_multi_room_ship_facilities_match_original_placements() -> Result {
    run(RoomCase::Multi)
}

fn run(room_case: RoomCase) -> Result {
    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))
        .map_err(|_| "open installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let entry = DataForgeEntry::read(&archive).map_err(|_| "read original DataForge")?;
    let forge = entry.open().map_err(|_| "open original DataForge")?;
    let (facilities, mut ships) =
        inventory::read(&forge).map_err(|_| "read candidate source inventory")?;
    let index = PathIndex::new(&archive);
    match room_case {
        RoomCase::Small => ships.sort_by_key(|ship| (ship.rooms.len(), ship.owner)),
        RoomCase::Multi => {
            ships.sort_by_key(|ship| (std::cmp::Reverse(ship.rooms.len()), ship.owner));
        }
    }
    let case = select(&archive, &index, &forge, &ships, &facilities, room_case)?;
    let label = room_case.label();
    report(&format!(
        "{label}: {} selected Record paths",
        case.selected_records
    ));
    let (catalog, normal) = build(&archive, &entry, &case.filter)?;
    assert_eq!(
        catalog.get("schemaVersion").and_then(Value::as_u64),
        Some(3),
        "the normal Catalog v3 wire schema"
    );
    crate::real_archive_shop_inventory_v3::verify_build(&archive, &catalog)?;
    comparison::compare(&catalog, &case, room_case)?;
    watch
        .check()
        .map_err(|_| "Archive changed during the comparison")?;
    report(&format!(
        "{label}: original facility join and physical positions verified; nested coverage={}, nested facility={}, normal={normal}",
        case.nested_coverage,
        case.witness.nested.is_some()
    ));
    Ok(())
}

fn build(archive: &Archive, entry: &DataForgeEntry, filter: &Filter) -> Result<(Value, bool)> {
    let options = CatalogOptions {
        filter: Some(filter),
        ..CatalogOptions::new(UNIX_EPOCH)
    };
    let (built, normal) = match build_catalog_from(archive, entry, &options) {
        Ok(built) => (built, true),
        Err(CatalogError::IncompleteExport(refusal)) => {
            let health = refusal.health();
            let failed_entries = health.failed_entries();
            let unrelated = !failed_entries.is_empty()
                && health.failed_records().is_empty()
                && failed_entries.iter().all(|entry| {
                    entry.operation() == EntryOperation::ShopInventory
                        && entry.code() == ExportDiagnosticCode::MalformedSource
                })
                && health.diagnostics().iter().all(|diagnostic| {
                    diagnostic.severity == ExportDiagnosticSeverity::Information
                        || (diagnostic.kind == Some(Kind::ShopInventories)
                            && diagnostic.code == ExportDiagnosticCode::MalformedSource)
                });
            if !unrelated {
                return Err(
                    "normal publication failed for a reason beyond malformed ShopInventory sources"
                        .into(),
                );
            }
            report(
                "Normal publication refused only for malformed ShopInventory sources; comparing explicit diagnostic output",
            );
            let options = CatalogOptions {
                publication_mode: CatalogPublicationMode::DiagnosticPartial,
                ..options
            };
            (
                build_catalog_from(archive, entry, &options)
                    .map_err(|_| "build filtered diagnostic catalog")?,
                false,
            )
        }
        Err(_) => return Err("build filtered normal catalog".into()),
    };
    let catalog = serde_json::from_slice(built.json()).map_err(|_| "read filtered Catalog JSON")?;
    Ok((catalog, normal))
}

fn report(line: &str) {
    crate::real_archive_verification::print_line(line);
}

struct Search<'archive, 'forge> {
    archive: &'archive Archive,
    index: &'archive PathIndex<'archive>,
    forge: &'forge DataForge<'forge>,
    facilities: &'forge std::collections::HashMap<p4kx_extraction::Guid, inventory::Facility>,
    attempts: usize,
    source_errors: usize,
    smallest_selection: Option<usize>,
}

impl Search<'_, '_> {
    fn ship<'ship>(
        &mut self,
        ship: &'ship inventory::Ship,
        room_case: RoomCase,
    ) -> Option<Case<'ship>> {
        let mut nested_coverage = false;
        if matches!(room_case, RoomCase::Multi) {
            for (component, room, path) in ship.rooms.iter().take(ROOM_SAMPLE) {
                let (witness, opened) = self.witness(path, true);
                nested_coverage |= opened > 0;
                if let Some(case) = witness.and_then(|witness| {
                    self.case(ship, *component, *room, witness, nested_coverage)
                }) {
                    return Some(case);
                }
            }
            if !nested_coverage && ship.rooms.len() < 2 {
                return None;
            }
        }
        for (component, room, path) in ship.rooms.iter().take(ROOM_SAMPLE) {
            let (witness, _) = self.witness(path, false);
            if let Some(case) = witness
                .and_then(|witness| self.case(ship, *component, *room, witness, nested_coverage))
            {
                return Some(case);
            }
        }
        None
    }

    fn witness(&mut self, path: &str, nested: bool) -> (Option<source::Witness>, usize) {
        self.attempts += 1;
        if let Ok(found) =
            source::find_witness(self.archive, self.index, path, self.facilities, nested)
        {
            found
        } else {
            self.source_errors += 1;
            (None, 0)
        }
    }

    fn case<'ship>(
        &mut self,
        ship: &'ship inventory::Ship,
        component: usize,
        room: usize,
        witness: source::Witness,
        nested_coverage: bool,
    ) -> Option<Case<'ship>> {
        let (filter, selected_records) =
            shared_filter(self.forge, &ship.path, &witness.configuration.path)?;
        self.smallest_selection = Some(
            self.smallest_selection
                .map_or(selected_records, |old| old.min(selected_records)),
        );
        (selected_records <= MAX_SELECTED_RECORDS).then_some(Case {
            ship,
            component,
            room,
            witness,
            filter,
            selected_records,
            nested_coverage,
        })
    }
}

fn select<'ship>(
    archive: &Archive,
    index: &PathIndex<'_>,
    forge: &DataForge<'_>,
    ships: &'ship [inventory::Ship],
    facilities: &std::collections::HashMap<p4kx_extraction::Guid, inventory::Facility>,
    room_case: RoomCase,
) -> Result<Case<'ship>> {
    let mut search = Search {
        archive,
        index,
        forge,
        facilities,
        attempts: 0,
        source_errors: 0,
        smallest_selection: None,
    };
    let candidates = ships
        .iter()
        .filter(|ship| matches!(room_case, RoomCase::Multi) || ship.rooms.len() <= SMALL_ROOM_COUNT)
        .take(SHIP_SAMPLE);
    for ship in candidates {
        if let Some(case) = search.ship(ship, room_case) {
            return Ok(case);
        }
    }
    report(&format!(
        "{}: attempted {} room reads; {} source errors; smallest matched filter {:?}",
        room_case.label(),
        search.attempts,
        search.source_errors,
        search.smallest_selection
    ));
    Err("no original facility with a shared Record filter within the test sample".into())
}

fn shared_filter(forge: &DataForge<'_>, ship: &str, facility: &str) -> Option<(Filter, usize)> {
    let ship = ship.replace('\\', "/").to_ascii_lowercase();
    let facility = facility.replace('\\', "/").to_ascii_lowercase();
    let prefix = ship
        .bytes()
        .zip(facility.bytes())
        .take_while(|(a, b)| a == b)
        .count();
    let remaining = ship.len().min(facility.len()).saturating_sub(prefix);
    let suffix = ship
        .bytes()
        .rev()
        .zip(facility.bytes().rev())
        .take(remaining)
        .take_while(|(a, b)| a == b)
        .count();
    let prefix = ship.get(..prefix)?;
    let suffix = ship.get(ship.len() - suffix..)?;
    let mut candidates = vec![format!("{prefix}*{suffix}")];
    let left = ship.split('/').next_back()?;
    let right = facility.split('/').next_back()?;
    for start in 0..left.len() {
        for end in start + 4..=left.len() {
            let Some(part) = left.get(start..end) else {
                continue;
            };
            if right.contains(part) && !part.contains(['*', '?']) {
                candidates.push(format!("{prefix}*{part}*{suffix}"));
            }
        }
    }
    candidates.sort_by_key(|pattern| std::cmp::Reverse(pattern.len()));
    candidates.dedup();
    let mut best = None;
    for pattern in candidates.iter().take(FILTER_CANDIDATES) {
        let Ok(filter) = Filter::parse(pattern) else {
            continue;
        };
        if !filter.matches(&ship) || !filter.matches(&facility) {
            continue;
        }
        let selected = forge
            .records()
            .filter(|record| record.record_path().is_ok_and(|path| filter.matches(path)))
            .count();
        if best.as_ref().is_none_or(|(_, old)| selected < *old) {
            best = Some((filter, selected));
        }
    }
    best
}
