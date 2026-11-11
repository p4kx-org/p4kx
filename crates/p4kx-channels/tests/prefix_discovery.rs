// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded prefix discovery from synthetic environments and configuration.

#[path = "prefix_discovery/config.rs"]
mod config;
#[path = "prefix_discovery/fake.rs"]
mod fake;
#[path = "prefix_discovery/limits.rs"]
mod limits;
#[path = "prefix_discovery/macos.rs"]
mod macos;

use std::error::Error;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use p4kx_channels::{DiscoveryIssueKind, DiscoveryLimits, WineEnvironment};

type TestResult = Result<(), Box<dyn Error>>;

const fn env(home: Option<PathBuf>) -> WineEnvironment {
    WineEnvironment::new(home, None)
}

fn library(fs: &mut fake::FakeFs, root: &Path, app: &str) -> PathBuf {
    let prefix = root.join("steamapps/compatdata").join(app).join("pfx");
    fs.directory(&prefix);
    prefix
}

#[test]
fn pure_seeds_cover_explicit_compat_native_flatpak_bottles_and_games_without_io() {
    let home = fake::root();
    let xdg = home.join("data");
    let custom = home.join("custom");
    let environment =
        WineEnvironment::new(Some(home.clone()), Some(custom.clone().into_os_string()))
            .with_compat_data(Some(home.join("compat").into_os_string()))
            .with_data_home(Some(xdg.clone()))
            .with_steam_roots(vec![home.join("steam-custom")])
            .with_steam_libraries(vec![home.join("external-library")])
            .with_bottles_roots(vec![home.join("bottles-custom")]);
    let seeds = environment.seeds();
    assert_eq!(
        seeds.direct_prefixes(),
        [
            custom,
            home.join("Games/star-citizen"),
            home.join("compat/pfx"),
        ],
        "direct prefix priority is explicit, legacy, then compat data"
    );
    assert!(
        seeds.steam_roots().contains(&xdg.join("Steam")),
        "XDG Steam is a pure seed"
    );
    assert!(
        seeds.steam_roots().contains(&home.join(".steam/steam")),
        "native Steam alias is seeded"
    );
    assert!(
        seeds
            .steam_roots()
            .contains(&home.join(".var/app/com.valvesoftware.Steam/data/Steam")),
        "the current Flatpak data root is seeded"
    );
    assert!(
        seeds
            .steam_roots()
            .contains(&home.join(".var/app/com.valvesoftware.Steam/.local/share/Steam")),
        "the legacy Flatpak data spelling is seeded"
    );
    assert!(
        seeds.bottles_roots().contains(&xdg.join("bottles/bottles")),
        "native Bottles is seeded"
    );
    assert!(
        seeds
            .bottles_roots()
            .contains(&home.join(".var/app/com.usebottles.bottles/data/bottles/bottles")),
        "Flatpak Bottles is seeded"
    );
    assert_eq!(
        seeds.games_roots(),
        [home.join("Games")],
        "Games discovery is one-level only"
    );
}

#[test]
fn explicit_compat_and_both_vdf_formats_find_multiple_external_libraries() -> TestResult {
    let home = fake::root();
    let steam = home.join("steam");
    let first = home.join("library one");
    let second = home.join("library two");
    let direct = home.join("compat/pfx");
    let mut fs = fake::FakeFs::default();
    fs.directory(&direct);
    let alpha = library(&mut fs, &first, "10");
    let beta = library(&mut fs, &second, "20");
    let old = format!(
        "\"LibraryFolders\" {{ \"1\" {} }}",
        serde_json::to_string(&first)?
    );
    let current = format!(
        "\"libraryfolders\" {{ \"2\" {{ \"path\" {} \"apps\" {{ \"20\" \"1\" }} }} }}",
        serde_json::to_string(&second)?
    );
    fs.config(&steam.join("config/libraryfolders.vdf"), old.into_bytes());
    fs.config(
        &steam.join("steamapps/libraryfolders.vdf"),
        current.into_bytes(),
    );
    let environment = env(None)
        .with_compat_data(Some(home.join("compat").into_os_string()))
        .with_steam_roots(vec![steam]);
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        report.is_complete(),
        "both valid formats preserve complete discovery: {:?}",
        report.issues()
    );
    assert_eq!(
        report.value(),
        &[direct, beta, alpha],
        "config priority and exact physical pfx paths are retained"
    );
    Ok(())
}

#[test]
fn bottles_and_games_are_one_level_and_duplicate_prefixes_keep_first_priority() {
    let home = fake::root();
    let bottles = home.join("bottles");
    let bottle = bottles.join("Work");
    let default = home.join("Games/star-citizen");
    let lutris = home.join("Games/lutris-sc");
    let nested = home.join("Games/lutris-sc/deeper/not-a-prefix");
    let mut fs = fake::FakeFs::default();
    for prefix in [&bottle, &default, &lutris, &nested] {
        fs.directory(prefix);
    }
    let environment = WineEnvironment::new(Some(home), Some(default.clone().into_os_string()))
        .with_bottles_roots(vec![bottles]);
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        report.is_complete(),
        "ordinary missing families are not failures"
    );
    assert_eq!(
        report.value(),
        &[default, bottle, lutris],
        "only immediate Games children are admitted once"
    );
    assert!(
        !fs.probes.borrow().iter().any(|probe| matches!(probe,
        fake::Probe::Directory(path) if path == &nested)),
        "no nested Games tree is enumerated"
    );
}

#[test]
fn invalid_relative_inputs_do_not_turn_into_current_directory_searches() {
    let environment =
        WineEnvironment::new(Some(PathBuf::from("relative")), Some(OsString::from("")))
            .with_compat_data(Some(OsString::from("relative-compat")))
            .with_data_home(Some(PathBuf::from("relative-data")));
    let fs = fake::FakeFs::default();
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        report.is_complete(),
        "ignored relative variables are ordinary absent candidates"
    );
    let empty: &[PathBuf] = &[];
    assert_eq!(
        report.value().as_slice(),
        empty,
        "no relative prefix is admitted"
    );
    assert_eq!(
        fs.probes.borrow().len(),
        0,
        "pure invalid inputs cause no filesystem operation"
    );
}

#[test]
fn exact_case_wins_and_a_unique_fallback_keeps_the_actual_spelling() {
    let root = fake::root();
    let exact = root.join("Prefix");
    let alternate = root.join("prefix");
    let mut fs = fake::FakeFs::default();
    fs.directory(&exact);
    fs.directory(&alternate);
    let report = WineEnvironment::new(None, Some(exact.clone().into_os_string()))
        .discover_with(&fs, DiscoveryLimits::default());
    assert_eq!(
        report.value(),
        &[exact],
        "an exact match wins despite a case-alternate sibling"
    );
    let requested = root.join("PREFIX");
    let ambiguous = WineEnvironment::new(None, Some(requested.into_os_string()))
        .discover_with(&fs, DiscoveryLimits::default());
    let empty: &[PathBuf] = &[];
    assert_eq!(
        ambiguous.value().as_slice(),
        empty,
        "two fallback spellings cannot choose a prefix"
    );
    assert!(
        ambiguous
            .issues()
            .iter()
            .any(|issue| matches!(issue.kind(), DiscoveryIssueKind::Ambiguous)),
        "the ambiguity remains a typed diagnostic"
    );
    let mut unique = fake::FakeFs::default();
    unique.directory(&alternate);
    let report = WineEnvironment::new(None, Some(root.join("PREFIX").into_os_string()))
        .discover_with(&unique, DiscoveryLimits::default());
    assert_eq!(
        report.value(),
        &[alternate],
        "a unique fallback returns its original spelling"
    );
}

#[test]
fn distinct_prefix_aliases_deduplicate_by_actual_resolved_identity() {
    let root = fake::root();
    let first = root.join("first");
    let alias = root.join("compat/pfx");
    let mut fs = fake::FakeFs::default();
    fs.directory(&first);
    fs.directory(&alias);
    fs.alias(&alias, &first);
    let environment = WineEnvironment::new(None, Some(first.clone().into_os_string()))
        .with_compat_data(Some(root.join("compat").into_os_string()));
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        report.is_complete(),
        "a proved directory alias is ordinary deduplication"
    );
    assert_eq!(
        report.value(),
        &[first],
        "the first spelling retains priority without a second prefix"
    );
}

#[test]
fn discovery_never_opens_credentials_or_unrelated_configuration() -> TestResult {
    let root = fake::root();
    let steam = root.join("steam");
    let config = steam.join("config/libraryfolders.vdf");
    let external = root.join("library");
    let mut fs = fake::FakeFs::default();
    let prefix = library(&mut fs, &external, "7");
    fs.config(
        &config,
        format!(
            "libraryfolders {{ 1 {} }}",
            serde_json::to_string(&external)?
        )
        .into_bytes(),
    );
    fs.config(
        &steam.join("config/loginData.json"),
        b"credentials".to_vec(),
    );
    fs.config(&prefix.join("user.reg"), b"registry".to_vec());
    let report = env(None)
        .with_steam_roots(vec![steam])
        .discover_with(&fs, DiscoveryLimits::default());
    assert!(
        report.is_complete(),
        "unrelated sibling files are outside discovery"
    );
    let opened: Vec<_> = fs
        .probes
        .borrow()
        .iter()
        .filter_map(|probe| match probe {
            fake::Probe::Config(path) => Some(path.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(
        opened,
        [config],
        "only the whitelisted Steam library configuration is opened"
    );
    Ok(())
}
