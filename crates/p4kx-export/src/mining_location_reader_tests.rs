// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Guid, Language, SourcedValue};

#[path = "mining_location_reader_fixture.rs"]
mod fixture;

use fixture::archive;

use crate::mining_locations::{
    ContainerInstanceKey, HarvestableOccurrenceRoute, MiningLocationOptions,
    MiningLocationSnapshot, ReachedHarvestableOccurrence, read_mining_locations,
    read_mining_locations_from,
};
use crate::{CatalogError, StarmapTraversalBudget};

const PLANET: &str = "10000000-0000-4000-8000-000000000001";
const PRESET: &str = "10000000-0000-4000-8000-000000000004";

type TestResult = Result<(), Box<dyn std::error::Error>>;

#[test]
fn reads_five_linked_tables_and_keeps_selected_locale_evidence() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir.path().join("Data.p4k"))?;
    let result = read_mining_locations(
        &archive,
        MiningLocationOptions::new(Language::English, None),
    )?;
    assert_eq!(
        result.mineable_elements.len(),
        1,
        "only the admitted elements"
    );
    assert_eq!(
        result.mineable_compositions.len(),
        1,
        "one linked composition"
    );
    assert_eq!(result.harvestable_provider_presets.len(), 1, "one provider");
    assert_eq!(
        result.starmap_objects.len(),
        1,
        "one admitted Starmap object"
    );
    assert_eq!(
        result.harvestable_locations.len(),
        2,
        "the reached and unreached physical containers both contribute"
    );
    check_links(&result)?;
    check_localization(&result)?;
    check_occurrences(&result)?;
    assert!(
        result.coverage.language_available
            && result.coverage.unreadable_record_bodies == 0
            && result.coverage.unreadable_harvest_containers == 0
            && result.coverage.locations_with_unresolved_parent == 0,
        "every selected synthetic input read successfully"
    );
    Ok(())
}

#[test]
fn caller_selected_child_budgets_keep_mining_coverage_incomplete() -> TestResult {
    let dir = tempfile::tempdir()?;
    let archive = archive(&dir.path().join("Data.p4k"))?;
    let mut options = MiningLocationOptions::new(Language::English, None);
    for budget in [
        StarmapTraversalBudget::new().with_source_children(0),
        StarmapTraversalBudget::new().with_placed_children_per_system(0),
    ] {
        options.starmap_budget = budget;
        let result = read_mining_locations(&archive, options)?;
        assert!(
            result.coverage.placement_inventory_incomplete,
            "selected quota cannot certify complete placement inventory"
        );
        assert!(
            result.coverage.unavailable_placement_occurrences > 0,
            "coverage reports the actual unvisited occurrence scope"
        );
    }
    Ok(())
}

#[test]
fn rejects_foreign_decoded_input_and_cancellation() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = archive(&dir.path().join("first.p4k"))?;
    let second = archive(&dir.path().join("second.p4k"))?;
    let first_entry = DataForgeEntry::read(&first)?;
    assert!(
        matches!(
            read_mining_locations_from(
                &second,
                &first_entry,
                MiningLocationOptions::new(Language::English, None),
            ),
            Err(CatalogError::SourceOriginMismatch)
        ),
        "matching bytes in another physical Archive cannot lend their provenance"
    );
    let cancel = CancelToken::default();
    cancel.cancel();
    assert!(
        matches!(
            read_mining_locations(
                &first,
                MiningLocationOptions::new(Language::English, Some(&cancel)),
            ),
            Err(CatalogError::Cancelled)
        ),
        "a cancelled refresh publishes no snapshot"
    );
    Ok(())
}

#[test]
#[ignore = "real Archive: set P4KX_DATA_P4K and pass --ignored"]
fn reads_installed_mining_locations_without_catalog_publication() -> TestResult {
    use std::io::{self, Write};
    use std::time::Instant;

    use p4kx_testkit::{real_archive_key, real_archive_path};

    let archive = Archive::open_with_key(real_archive_path()?, Some(real_archive_key()?))?;
    let started = Instant::now();
    let snapshot = read_mining_locations(
        &archive,
        MiningLocationOptions::new(Language::English, None),
    )?;
    let counts = [
        snapshot.mineable_elements.len(),
        snapshot.mineable_compositions.len(),
        snapshot.harvestable_provider_presets.len(),
        snapshot.harvestable_locations.len(),
        snapshot.starmap_objects.len(),
    ];
    assert!(
        counts.iter().all(|&count| count > 0),
        "the installed Archive supplies all five tables"
    );
    assert!(
        snapshot.coverage.language_available,
        "the selected locale was loaded"
    );
    assert!(
        !snapshot.harvestable_occurrences.is_empty(),
        "the selected routes are present"
    );
    let build = snapshot.build.as_ref().ok_or("installed Build identity")?;
    writeln!(
        io::stdout(),
        "Mining projection change={} read_ms={} table_counts={counts:?} occurrences={} uses={} unreadable_socs={}",
        build.change_number(),
        started.elapsed().as_millis(),
        snapshot.harvestable_occurrences.len(),
        snapshot.source_inventory.use_count(),
        snapshot.coverage.unreadable_harvest_containers,
    )?;
    Ok(())
}
fn check_composition(result: &MiningLocationSnapshot) -> TestResult {
    let element = result.mineable_elements.first().ok_or("element")?;
    let composition = result.mineable_compositions.first().ok_or("composition")?;
    let slot = composition
        .parts
        .value()
        .and_then(|parts| parts.first())
        .ok_or("authored slot")?;
    let part = slot.value().ok_or("authored share")?;
    let link = part.mineable_element.value().ok_or("element reference")?;
    assert_eq!(
        link.guid().value(),
        element.source.guid.value(),
        "the composition preserves its original reference"
    );
    assert!(
        matches!(
            part.min_percentage.value(),
            Some(crate::resource_number::ResourceNumber::Single(value))
                if value.raw_bits() == 20.0_f32.to_bits()
        ),
        "the authored share keeps its value and type"
    );
    assert!(
        closed(&composition.parts, result)
            && closed(slot, result)
            && closed(&part.min_percentage, result),
        "array, slot and field source use references resolve"
    );
    assert_eq!(
        part.min_percentage.origin().contains_encrypted_source(),
        Some(false),
        "the SDK proves clear Game2.dcb provenance"
    );
    assert!(
        closed(&element.source.guid, result),
        "header source use references resolve"
    );
    Ok(())
}

fn check_links(result: &MiningLocationSnapshot) -> TestResult {
    check_composition(result)?;
    let provider = result
        .harvestable_provider_presets
        .first()
        .ok_or("provider")?;
    let planet = result.starmap_objects.first().ok_or("planet")?;
    let planet_guid: Guid = PLANET.parse()?;
    assert!(
        closed(&planet.position, result),
        "position source use references resolve"
    );
    for row in &result.harvestable_locations {
        assert_eq!(
            Some(&row.preset_guid),
            provider.source.guid.value(),
            "each location links to the admitted provider"
        );
        if row.container.ends_with("tile.socpak") {
            assert_eq!(
                row.starmap_object_guid,
                Some(planet_guid),
                "nearest Starmap parent"
            );
        } else {
            assert!(
                row.container.ends_with("unreached.socpak") && row.starmap_object_guid.is_none(),
                "the unreached location has no authored parent"
            );
        }
    }
    Ok(())
}

fn check_occurrences(result: &MiningLocationSnapshot) -> TestResult {
    let mut reached = Vec::new();
    let mut unreached = 0;
    for row in &result.harvestable_occurrences {
        assert!(
            closed(&row.preset_guid, result),
            "verified SOC token uses resolve"
        );
        assert!(
            closed(&row.container, result),
            "the physical Entry has source evidence"
        );
        assert!(
            closed(&row.container_path, result),
            "the physical Entry path has source evidence"
        );
        assert!(
            result.harvestable_locations.iter().any(|logical| {
                Some(logical.container.as_str()) == row.container_path.value().map(String::as_str)
                    && logical.system == row.system
                    && Some(&logical.preset_guid) == row.preset_guid.value()
                    && logical.starmap_object_guid == row.starmap_object_guid
            }),
            "each physical row joins to the legacy logical location"
        );
        match &row.route {
            HarvestableOccurrenceRoute::Reached(route) => {
                reached.push((row, check_reached_route(route, result)?));
            }
            HarvestableOccurrenceRoute::Unreached => {
                assert!(
                    row.starmap_object_guid.is_none(),
                    "no inferred route or parent"
                );
                unreached += 1;
            }
        }
    }
    let [(first, first_id), (second, second_id)] = reached.as_slice() else {
        return Err("expected two reached routes to the same container".into());
    };
    assert_eq!(
        first.container.value(),
        second.container.value(),
        "one physical Entry"
    );
    assert_eq!(
        first.preset_guid.origin(),
        second.preset_guid.origin(),
        "both routes use the same verified physical token ranges"
    );
    assert_ne!(first_id, second_id, "separate authored Child routes");
    assert_eq!(
        unreached, 1,
        "the independent inventory-only Entry has no route"
    );
    Ok(())
}

fn check_reached_route<'route>(
    route: &'route ReachedHarvestableOccurrence,
    result: &MiningLocationSnapshot,
) -> Result<&'route ContainerInstanceKey, Box<dyn std::error::Error>> {
    for value in [&route.id, &route.parent] {
        assert!(closed(value, result), "route and parent selectors resolve");
    }
    assert!(
        closed(&route.root, result),
        "physical system root selectors resolve"
    );
    assert!(
        closed(&route.containing_record_guid, result),
        "original owner selector resolves"
    );
    let root = route.root.value().ok_or("root identity")?;
    let id = route.id.value().ok_or("physical occurrence ID")?;
    let parent = route.parent.value().ok_or("direct root parent")?;
    assert_eq!(
        id.root_entry_index(),
        root.entry_index(),
        "the route uses the exact root"
    );
    assert_eq!(
        parent.root_entry_index(),
        root.entry_index(),
        "the parent uses the same root"
    );
    assert!(
        parent.child_node_indices().is_empty(),
        "parent is the root container"
    );
    Ok(id)
}

fn check_localization(result: &MiningLocationSnapshot) -> TestResult {
    let object = result.starmap_objects.first().ok_or("Starmap object")?;
    let name = object.name.value().ok_or("canonical name ID")?;
    let row = result
        .game_texts
        .iter()
        .find(|text| text.id.value() == Some(name))
        .ok_or("name resolves into protected selected text")?;
    let translations = row.translations.value().ok_or("text translations")?;
    assert_eq!(
        translations.len(),
        1,
        "only the requested locale is consulted"
    );
    let english = translations
        .get(Language::English.code())
        .ok_or("requested locale")?;
    assert_eq!(
        english.display_text.value().map(String::as_str),
        Some("Synthetic planet"),
        "legacy String name is resolved through the canonical Catalog policy"
    );
    assert!(
        closed(&english.display_text, result),
        "localized value's source uses resolve"
    );
    Ok(())
}

fn closed<T>(value: &SourcedValue<T>, result: &MiningLocationSnapshot) -> bool {
    let references = value.origin().source_uses_refs();
    !references.is_empty()
        && value.origin().source_set_complete()
        && references.iter().all(|reference| {
            result
                .source_inventory
                .uses()
                .iter()
                .any(|used| used.id() == reference)
        })
}
