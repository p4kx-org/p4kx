// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs::{self, File, FileTimes};
use std::path::Path;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use p4kx_channels::{DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment};

use super::LibraryProtectionCache;
use crate::Installation;

use super::fixture::{self, Fixture, TestResult};

const FIXTURE_EPOCH_SECONDS: u64 = 1_792_000_000;

#[test]
fn prefix_only_log_updates_protect_new_libraries_and_never_unprotect_old_ones() -> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let a = Fixture::library(&prefix, "A")?;
    let b = Fixture::library(&prefix, "B")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    fixture::libraries(&log, &["A"])?;
    let recipe = Fixture::recipe(vec![prefix]);
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    assert_eq!(
        cache.refresh(&recipe.snapshot(), &mut installation).len(),
        0,
        "prefix-only discovery is complete without a native launcher"
    );
    assert!(
        installation.check(&a.join("LIVE/new.bin")).is_err(),
        "the initial prefix library is protected"
    );
    assert!(
        installation.check(&b.join("LIVE/new.bin")).is_ok(),
        "an unrelated library is not guessed"
    );
    fixture::libraries(&log, &["A", "B"])?;
    assert_eq!(
        cache.refresh(&recipe.snapshot(), &mut installation).len(),
        0,
        "changing only a prefix log refreshes a complete source set"
    );
    assert!(
        installation.check(&b.join("LIVE/new.bin")).is_err(),
        "the newly logged library is protected"
    );
    fs::remove_file(&log)?;
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation.check(&a.join("LIVE/new.bin")).is_err(),
        "removing a log never removes old protection"
    );
    assert!(
        installation.check(&b.join("LIVE/new.bin")).is_err(),
        "all observed libraries remain protected"
    );
    assert!(
        installation
            .check(&fixture.root().join("export/new.bin"))
            .is_ok(),
        "an export sibling remains writable"
    );
    Ok(())
}

#[test]
fn creating_a_new_user_and_a_new_prefix_refreshes_the_same_monotone_cache() -> TestResult {
    let fixture = Fixture::new()?;
    let first = fixture.prefix("Games/star-citizen")?;
    let a = Fixture::library(&first, "A")?;
    let recipe = fixture.dynamic();
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    refresh(&mut cache, &recipe, &mut installation)?;
    let alice = Fixture::log(&first, "Alice")?;
    fixture::libraries(&alice, &["A"])?;
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation.check(&a.join("LIVE/result.bin")).is_err(),
        "a newly created user source invalidates the empty source set"
    );
    let second = fixture.prefix("Games/another-prefix")?;
    let b = Fixture::library(&second, "B")?;
    let bob = Fixture::log(&second, "Bob")?;
    fixture::libraries(&bob, &["B"])?;
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation.check(&b.join("LIVE/result.bin")).is_err(),
        "a prefix created after the first snapshot is discovered"
    );
    fs::remove_file(&alice)?;
    fs::remove_file(&bob)?;
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation.check(&a.join("LIVE/result.bin")).is_err(),
        "source removal preserves A"
    );
    assert!(
        installation.check(&b.join("LIVE/result.bin")).is_err(),
        "source removal preserves B"
    );
    Ok(())
}

#[test]
fn shared_log_bytes_are_interpreted_in_both_wine_contexts_without_losing_protection() -> TestResult
{
    let fixture = Fixture::new()?;
    let first = fixture.prefix("first")?;
    let second = fixture.prefix("second")?;
    let a = Fixture::library(&first, "A")?;
    let alias_a = Fixture::library(&second, "A")?;
    let first_log = Fixture::log(&first, "Pilot")?;
    let second_log = Fixture::log(&second, "Pilot")?;
    fixture::libraries(&first_log, &["A"])?;
    fs::hard_link(&first_log, &second_log)?;
    let recipe = Fixture::recipe(vec![first, second]);
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation.check(&a.join("LIVE/result.bin")).is_err(),
        "the first actual prefix interpretation is protected"
    );
    assert!(
        installation
            .check(&alias_a.join("LIVE/result.bin"))
            .is_err(),
        "physical log deduplication does not discard a different prefix interpretation"
    );
    fs::remove_file(&second_log)?;
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation
            .check(&alias_a.join("LIVE/result.bin"))
            .is_err(),
        "removing an alias occurrence does not unprotect its already observed library"
    );
    Ok(())
}

#[test]
fn partial_discovery_cannot_keep_an_old_complete_stamp_after_equal_stats_recovery() -> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let a = Fixture::library(&prefix, "A")?;
    let b = Fixture::library(&prefix, "B")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    fixture::libraries(&log, &["A"])?;
    let fixed_time = UNIX_EPOCH + Duration::from_secs(FIXTURE_EPOCH_SECONDS);
    set_mtime(&log, fixed_time)?;
    let steam = fixture.root().join("steam");
    fs::create_dir_all(steam.join("steamapps"))?;
    let config = steam.join("steamapps/libraryfolders.vdf");
    fs::write(&config, "\"libraryfolders\" {}")?;
    set_mtime(&config, fixed_time)?;
    let recipe = SearchRecipe::with_environment(
        SearchPaths::default(),
        WineEnvironment::new(None, Some(prefix.into_os_string())).with_steam_roots(vec![steam]),
        DiscoveryLimits::default(),
    );
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    assert_eq!(
        cache.refresh(&recipe.snapshot(), &mut installation).len(),
        0,
        "the initial complete source is admitted"
    );
    fs::write(&config, "\"libraryfolders\" { ")?;
    set_mtime(&config, fixed_time)?;
    assert_ne!(
        cache.refresh(&recipe.snapshot(), &mut installation).len(),
        0,
        "a malformed equal-size configuration is explicitly incomplete"
    );
    fixture::libraries(&log, &["B"])?;
    set_mtime(&log, fixed_time)?;
    fs::write(&config, "\"libraryfolders\" {}")?;
    set_mtime(&config, fixed_time)?;
    assert_eq!(
        cache.refresh(&recipe.snapshot(), &mut installation).len(),
        0,
        "a repaired current source is read rather than reusing old admission"
    );
    assert!(
        installation.check(&a.join("LIVE/new.bin")).is_err(),
        "old A remains protected"
    );
    assert!(
        installation.check(&b.join("LIVE/new.bin")).is_err(),
        "equal-stat recovery still protects newly observed B"
    );
    Ok(())
}

fn set_mtime(path: &Path, time: SystemTime) -> TestResult {
    File::options()
        .write(true)
        .open(path)?
        .set_times(FileTimes::new().set_modified(time))?;
    Ok(())
}

#[test]
fn equal_stat_file_replacement_refreshes_protection_for_a_different_library() -> TestResult {
    let fixture = Fixture::new()?;
    let prefix = fixture.prefix("prefix")?;
    let a = Fixture::library(&prefix, "A")?;
    let b = Fixture::library(&prefix, "B")?;
    let log = Fixture::log(&prefix, "Pilot")?;
    fixture::libraries(&log, &["A"])?;
    let fixed_time = UNIX_EPOCH + Duration::from_secs(FIXTURE_EPOCH_SECONDS);
    set_mtime(&log, fixed_time)?;
    let recipe = Fixture::recipe(vec![prefix]);
    let mut cache = LibraryProtectionCache::new();
    let mut installation = Installation::new();
    refresh(&mut cache, &recipe, &mut installation)?;
    let old_size = fs::metadata(&log)?.len();
    let old_time = fs::metadata(&log)?.modified()?;
    let replacement = log.with_extension("replacement");
    fixture::libraries(&replacement, &["B"])?;
    set_mtime(&replacement, old_time)?;
    assert_eq!(
        fs::metadata(&replacement)?.len(),
        old_size,
        "the replacement keeps the old length"
    );
    assert_eq!(
        fs::metadata(&replacement)?.modified()?,
        old_time,
        "the replacement keeps the old timestamp"
    );
    fs::remove_file(&log)?;
    fs::rename(&replacement, &log)?;
    refresh(&mut cache, &recipe, &mut installation)?;
    assert!(
        installation.check(&a.join("LIVE/new.bin")).is_err(),
        "old A remains protected"
    );
    assert!(
        installation.check(&b.join("LIVE/new.bin")).is_err(),
        "opened-file identity makes the equal-stat replacement observable"
    );
    Ok(())
}

fn refresh(
    cache: &mut LibraryProtectionCache,
    recipe: &SearchRecipe,
    installation: &mut Installation,
) -> TestResult {
    let issues = cache.refresh(&recipe.snapshot(), installation);
    if !issues.is_empty() {
        return Err(format!("expected complete synthetic discovery: {issues:?}").into());
    }
    Ok(())
}
