// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry, Language};

#[path = "mining_location_reader_assertions.rs"]
mod assertions;

#[path = "mining_location_reader_fixture.rs"]
mod fixture;

use fixture::{PLANET, archive};

use crate::mining_locations::{
    MiningLocationOptions, read_mining_locations, read_mining_locations_from,
};
use crate::{CatalogError, StarmapTraversalBudget};

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
    assertions::check_links(&result)?;
    assertions::check_localization(&result)?;
    assertions::check_occurrences(&result)?;
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
fn proved_absence_counts_each_route_without_blocking_complete_mining_placements() -> TestResult {
    const MISSING: &str = "Data/ObjectContainers/PU/system/t/absent.socpak";
    const UNSUPPORTED: &str = "C:/authoring/unsearchable.socpak";
    let dir = tempfile::tempdir()?;
    for (name, targets, unavailable) in [
        ("proved", [MISSING].as_slice(), 0),
        ("mixed", [MISSING, UNSUPPORTED].as_slice(), 2),
    ] {
        let archive = fixture::archive_with_external_targets(
            &dir.path().join(format!("{name}.p4k")),
            targets,
        )?;
        let result = read_mining_locations(
            &archive,
            MiningLocationOptions::new(Language::English, None),
        )?;
        assert_eq!(
            result.coverage.verified_missing_placement_occurrences, 2,
            "{name}: one declared absence on each of the two tile routes"
        );
        assert_eq!(
            result.coverage.unavailable_placement_occurrences, unavailable,
            "{name}: unverified targets remain unavailable"
        );
        assert_eq!(
            result.coverage.has_complete_placement_inventory(),
            unavailable == 0,
            "{name}: cache reconciliation requires complete placement coverage"
        );
        assertions::check_occurrences(&result)?;
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
