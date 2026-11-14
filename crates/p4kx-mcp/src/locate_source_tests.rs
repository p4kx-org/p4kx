// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File};
use std::io::Write;
use std::sync::Arc;

use p4kx_channels::{SearchPaths, SearchRecipe};
use p4kx_settings::{Settings, WineLocations};
use p4kx_testkit::write_build_manifest;

use super::Locations;
use crate::discovery_notes;

#[path = "locate_source_fixture.rs"]
mod fixture;
use fixture::{Fixture, TestResult};

#[test]
fn prefix_only_patches_reuse_then_invalidate_on_file_creation_change_and_removal() -> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    let locations = Locations::with_recipe(Fixture::explicit(vec![prefix]), None);
    let empty = locations.public_patches();
    fixture::assert_patch(&empty, None);
    assert!(
        Arc::ptr_eq(&empty, &locations.public_patches()),
        "a complete unchanged absence is reusable"
    );
    fixture::patch(&log, "4.3.1")?;
    fixture::set_mtime(&log, fixture::time(0))?;
    let first = locations.public_patches();
    fixture::assert_patch(&first, Some("4.3.1"));
    assert!(
        !Arc::ptr_eq(&empty, &first),
        "a newly created prefix log invalidates absence"
    );
    assert!(
        Arc::ptr_eq(&first, &locations.public_patches()),
        "an unchanged successful scan reuses its Arc"
    );
    fixture::patch(&log, "4.4.1")?;
    fixture::set_mtime(&log, fixture::time(10))?;
    let changed = locations.public_patches();
    fixture::assert_patch(&changed, Some("4.4.1"));
    assert!(
        !Arc::ptr_eq(&first, &changed),
        "same-length content with a changed timestamp is reread"
    );
    fs::remove_file(&log)?;
    let removed = locations.public_patches();
    fixture::assert_patch(&removed, None);
    assert!(
        !Arc::ptr_eq(&changed, &removed),
        "removing a log invalidates its old patch map"
    );
    Ok(())
}

#[test]
fn equal_size_and_mtime_replacement_has_a_different_opened_file_identity() -> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    fixture::patch(&log, "4.3.1")?;
    fixture::set_mtime(&log, fixture::time(0))?;
    let locations = Locations::with_recipe(Fixture::explicit(vec![prefix]), None);
    let before = locations.public_patches();
    let old_length = fs::metadata(&log)?.len();
    let old_time = fixture::mtime(&log)?;
    let replacement = log.with_extension("replacement");
    fixture::patch(&replacement, "4.4.1")?;
    fixture::set_mtime(&replacement, old_time)?;
    assert_eq!(
        fs::metadata(&replacement)?.len(),
        old_length,
        "the replacement defeats a size-only check"
    );
    assert_eq!(
        fixture::mtime(&replacement)?,
        old_time,
        "the replacement defeats a timestamp-only check"
    );
    fs::remove_file(&log)?;
    fs::rename(&replacement, &log)?;
    let after = locations.public_patches();
    fixture::assert_patch(&after, Some("4.4.1"));
    assert!(
        !Arc::ptr_eq(&before, &after),
        "a different actual file cannot reuse the old map"
    );
    Ok(())
}

#[test]
fn current_user_and_prefix_topology_is_not_frozen_at_locations_creation() -> TestResult {
    let fixture = Fixture::new()?;
    let steam = fixture.path().join("steam");
    fs::create_dir_all(steam.join("steamapps/compatdata"))?;
    let locations = Locations::with_recipe(fixture.environment(&steam), None);
    fixture::assert_patch(&locations.public_patches(), None);
    let prefix = fixture.prefix("prefix")?;
    let first = Fixture::log(&prefix, "Pilot")?;
    fixture::patch(&first, "4.3.1")?;
    fixture::assert_patch(&locations.public_patches(), Some("4.3.1"));
    let new_prefix = steam.join("steamapps/compatdata/901/pfx");
    fs::create_dir_all(new_prefix.join("drive_c"))?;
    let later = Fixture::log(&new_prefix, "LaterPilot")?;
    fixture::patch(&later, "4.4.1")?;
    fixture::assert_patch(&locations.public_patches(), Some("4.4.1"));
    fs::remove_file(&later)?;
    fixture::assert_patch(&locations.public_patches(), Some("4.3.1"));
    Ok(())
}

#[test]
fn an_incomplete_discovery_discards_previous_admission_and_recovery_rereads_equal_stats()
-> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    fixture::patch(&log, "4.3.1")?;
    fixture::set_mtime(&log, fixture::time(0))?;
    let steam = fixture.path().join("steam");
    fs::create_dir_all(steam.join("steamapps"))?;
    let config = steam.join("steamapps/libraryfolders.vdf");
    fs::write(&config, "\"libraryfolders\" {}")?;
    let locations = Locations::with_recipe(fixture.environment(&steam), None);
    let original = locations.public_patches();
    let config_time = fixture::mtime(&config)?;
    fs::write(&config, "\"libraryfolders\" { ")?;
    fixture::set_mtime(&config, config_time)?;
    discovery_notes::clear();
    let partial = locations.public_patches();
    fixture::assert_patch(&partial, Some("4.3.1"));
    assert_ne!(
        discovery_notes::take(),
        Vec::<String>::new(),
        "incomplete discovery is surfaced as a result notice"
    );
    assert!(
        !Arc::ptr_eq(&original, &partial),
        "partial discovery cannot reuse an earlier complete admission"
    );
    let same_time = fixture::mtime(&log)?;
    fixture::patch(&log, "4.4.1")?;
    fixture::set_mtime(&log, same_time)?;
    fs::write(&config, "\"libraryfolders\" {}")?;
    fixture::set_mtime(&config, config_time)?;
    discovery_notes::clear();
    let repaired = locations.public_patches();
    fixture::assert_patch(&repaired, Some("4.4.1"));
    assert_eq!(
        discovery_notes::take(),
        Vec::<String>::new(),
        "a repaired source does not inherit old diagnostics"
    );
    assert!(
        Arc::ptr_eq(&repaired, &locations.public_patches()),
        "successful recovery becomes reusable"
    );
    Ok(())
}

#[test]
fn removing_an_alias_occurrence_changes_the_winner_with_identical_primary_files() -> TestResult {
    let fixture = Fixture::new()?;
    let native = fixture.path().join("native");
    fs::create_dir_all(native.join("logs"))?;
    let native_log = native.join("logs/log.log");
    fixture::patch(&native_log, "4.3.1")?;
    let competing = fixture.prefix("competing")?;
    let competing_log = Fixture::log(&competing, "Pilot")?;
    fixture::patch(&competing_log, "4.4.1")?;
    let alias = fixture.prefix("alias")?;
    let alias_log = Fixture::log(&alias, "Pilot")?;
    fs::hard_link(&native_log, &alias_log)?;
    let locations = Locations::with_recipe(
        SearchRecipe::explicit(SearchPaths {
            launcher: Some(native),
            wine_prefixes: vec![competing, alias],
            ..SearchPaths::default()
        }),
        None,
    );
    let before = locations.public_patches();
    fixture::assert_patch(&before, Some("4.3.1"));
    fs::remove_file(&alias_log)?;
    let after = locations.public_patches();
    fixture::assert_patch(&after, Some("4.4.1"));
    assert!(
        !Arc::ptr_eq(&before, &after),
        "ordered alias context is part of cache identity"
    );
    Ok(())
}

#[test]
fn appending_a_new_patch_line_refreshes_the_same_locations_instance() -> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    fixture::patch(&log, "4.3.1")?;
    let locations = Locations::with_recipe(Fixture::explicit(vec![prefix]), None);
    let before = locations.public_patches();
    let old_size = fs::metadata(&log)?.len();
    let mut file = File::options().append(true).open(&log)?;
    writeln!(file, "Delta update applied (SC LIVE 4.4.1-live.10000001)")?;
    file.flush()?;
    assert!(
        fs::metadata(&log)?.len() > old_size,
        "the appended line changes source length"
    );
    let after = locations.public_patches();
    fixture::assert_patch(&after, Some("4.4.1"));
    assert!(
        !Arc::ptr_eq(&before, &after),
        "append invalidates the complete cached map"
    );
    Ok(())
}

#[test]
fn saved_cross_over_roots_are_shared_by_channels_and_the_patch_cache() -> TestResult {
    discovery_notes::clear();
    let fixture = Fixture::new()?;
    let bottle = fixture.prefix("bottles/Shared")?;
    let log = Fixture::log(&bottle, "Pilot")?;
    fixture::patch(&log, "4.4.1")?;
    let live = bottle.join("drive_c/Program Files/Roberts Space Industries/StarCitizen/LIVE");
    fs::create_dir_all(&live)?;
    fs::write(live.join("Data.p4k"), [])?;
    write_build_manifest(
        &live,
        "sc-alpha-4.4.0",
        "4.4.0.0",
        "10000001",
        "synthetic-build",
    )?;
    let file = fixture.path().join("settings.toml");
    let settings = Settings {
        wine: WineLocations::new(Vec::new(), vec![fixture.path().join("bottles")]),
        ..Settings::default()
    };
    settings.save(&file)?;
    let locations = Locations::with_recipe(Fixture::explicit(Vec::new()), Some(file.clone()));

    let channels = locations.channels()?;
    let [channel] = channels.as_slice() else {
        return Err("one saved-root Channel".into());
    };
    assert!(
        channel.build().and_then(|build| build.public_patch()) == Some("4.4.1"),
        "the Channel joins the patch from its admitted bottle"
    );
    let complete = locations.public_patches();
    fixture::assert_patch(&complete, Some("4.4.1"));

    fs::write(&file, "[wine")?;
    assert!(
        locations.channels().is_err(),
        "the Channel settings error remains explicit"
    );
    let partial = locations.public_patches();
    fixture::assert_patch(&partial, None);
    assert!(
        !discovery_notes::take().is_empty(),
        "a malformed profile prevents complete patch-source evidence"
    );

    settings.save(&file)?;
    assert!(
        Arc::ptr_eq(&complete, &locations.public_patches()),
        "the optional fallback was not admitted as a replacement complete cache entry"
    );
    Ok(())
}
