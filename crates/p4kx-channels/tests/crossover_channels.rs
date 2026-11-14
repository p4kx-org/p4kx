// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Exercises shared Channel and patch discovery through a `CrossOver` bottle.

use std::error::Error;
use std::fs;
use std::path::Path;

use p4kx_channels::{SearchPaths, SearchRecipe};
use p4kx_settings::{Settings, WineLocations};
use p4kx_testkit::write_build_manifest;

type TestResult = Result<(), Box<dyn Error>>;

#[test]
fn crossover_sources_reuse_channel_patch_and_refresh_stamp() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let roots = temporary.path().join("Bottle Roots");
    fs::create_dir_all(&roots)?;
    let recipe = SearchRecipe::explicit(SearchPaths::default())
        .with_wine_locations(Vec::new(), vec![roots.clone()]);
    let before = recipe.snapshot();
    assert!(
        before.paths().wine_prefixes.is_empty(),
        "the explicit recipe discovers no missing bottle and reads no process environment"
    );
    let bottle = roots.join("Synthetic Bottle");
    let live = bottle.join("drive_c/Program Files/Roberts Space Industries/StarCitizen/LIVE");
    let ptu = bottle.join("drive_c/Games/StarCitizen/PTU");
    channel(&live, "1001")?;
    channel(&ptu, "1002")?;
    let logs = bottle.join("drive_c/users/Tester/AppData/Roaming/rsilauncher/logs");
    fs::create_dir_all(&logs)?;
    let current = logs.join("log.log");
    fs::write(
        &current,
        concat!(
            "{ \"message\": \"Installing Star Citizen PTU at C:\\\\Games\\\\StarCitizen (force DP false)\" }\n",
            "(SC PTU 5.0.0-ptu.1002)\n",
        ),
    )?;
    let opened = recipe.snapshot();
    assert!(
        opened.stamp() != before.stamp(),
        "the new physical bottle refreshes the stamp"
    );
    check_channels(&opened, &live, &ptu);
    fs::remove_file(&current)?;
    let removed = recipe.snapshot();
    assert!(
        removed.stamp() != opened.stamp(),
        "removing the only log changes the source stamp"
    );
    let report = removed.detect();
    assert!(report.is_complete(), "a missing log is normal absence");
    assert_eq!(
        report.value().channels().len(),
        1,
        "the default library remains"
    );
    assert!(
        report.value().patches().get("1002").is_none(),
        "the old patch is invalidated"
    );
    Ok(())
}

fn channel(path: &Path, change: &str) -> TestResult {
    fs::create_dir_all(path)?;
    fs::write(path.join("Data.p4k"), [])?;
    write_build_manifest(path, "sc-alpha-5.0.0", "5.0.0.0", change, "synthetic-build")?;
    Ok(())
}

fn check_channels(snapshot: &p4kx_channels::LauncherSnapshot, live: &Path, ptu: &Path) {
    let report = snapshot.detect();
    assert!(
        report.is_complete(),
        "the admitted default and log libraries are both readable"
    );
    assert_eq!(
        report.value().channels().len(),
        2,
        "both actual synthetic libraries survive"
    );
    for original in [live, ptu] {
        let expected = original.join("Data.p4k");
        assert!(
            report
                .value()
                .channels()
                .iter()
                .any(|channel| channel.archive() == expected),
            "the shared scanner retains the actual Archive path"
        );
    }
    assert_eq!(
        report.value().patches().get("1002"),
        Some("5.0.0"),
        "the same scan resolves the patch"
    );
}

#[test]
fn the_saved_profile_roots_refresh_the_same_explicit_recipe() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let file = temporary.path().join("settings.toml");
    let recipe = SearchRecipe::explicit(SearchPaths::default());
    let unset = Settings::load(&file)?;
    assert!(
        unset.wine.is_empty(),
        "an older or missing profile defaults to no saved sources"
    );
    let before = recipe.snapshot_with_settings(&unset);

    let first = temporary.path().join("one");
    let second = temporary.path().join("two");
    let explicit = temporary.path().join("configured-wine");
    let original = first.join("Bottle");
    let replacement = second.join("Replacement");
    for path in [&explicit, &original, &replacement] {
        fs::create_dir_all(path.join("drive_c"))?;
    }
    let mut settings = Settings {
        wine: WineLocations::new(vec![explicit.clone()], vec![first]),
        ..Settings::default()
    };
    settings.save(&file)?;
    let current = recipe.snapshot_with_settings(&Settings::load(&file)?);
    assert_eq!(
        current.paths().wine_prefixes.as_slice(),
        [explicit.clone(), original],
        "the persisted profile contributes the explicit prefix and existing bottle"
    );
    settings.wine.crossover_bottle_roots = vec![second];
    settings.save(&file)?;
    let refreshed = recipe.snapshot_with_settings(&Settings::load(&file)?);
    assert_eq!(
        refreshed.paths().wine_prefixes.as_slice(),
        [explicit, replacement],
        "changing the saved bottle root replaces its source without resetting the recipe"
    );
    assert!(
        current.stamp() != before.stamp() && refreshed.stamp() != current.stamp(),
        "current source stamps reflect the actual saved root topology"
    );
    Ok(())
}
