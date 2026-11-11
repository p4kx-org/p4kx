// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::Cell;
use std::fs;
use std::path::Path;

use crate::detect::SearchPaths;
use crate::search_recipe::SearchRecipe;

use crate::launcher_test_support as support;
use support::TestResult;

fn sources(root: &Path) -> TestResult<SearchRecipe> {
    let native = root.join("native");
    fs::create_dir_all(native.join("logs"))?;
    fs::write(
        native.join("logs/log.old.log"),
        support::patch_change("4.0.0", "10") + &support::patch_change("4.0.0", "11"),
    )?;
    fs::write(
        native.join("logs/log.log"),
        support::patch_change("4.0.1", "10") + &support::patch("4.0.1"),
    )?;
    let first = root.join("first");
    let zed = support::launcher(&first, "Zed")?;
    let alpha = support::launcher(&first, "Alpha")?;
    fs::write(
        alpha.join("log.old.log"),
        support::patch_change("5.0.0", "20"),
    )?;
    fs::write(
        alpha.join("log.log"),
        support::patch_change("5.0.1", "20")
            + &support::patch_change("5.0.1", "21")
            + &support::patch("5.0.1"),
    )?;
    fs::write(
        zed.join("log.log"),
        support::patch_change("5.0.2", "21") + &support::patch("5.0.2"),
    )?;
    let second = root.join("second");
    fs::write(
        support::launcher(&second, "Pilot")?.join("log.log"),
        support::patch("6.0.0"),
    )?;
    Ok(SearchRecipe::explicit(SearchPaths {
        launcher: Some(native),
        wine_prefixes: vec![first, second],
        ..SearchPaths::default()
    }))
}

#[test]
fn standard_selection_uses_native_wine_user_and_old_current_priority_without_host_environment()
-> TestResult {
    let temporary = tempfile::tempdir()?;
    let recipe = sources(temporary.path())?;
    let report = super::standard_sources(None, || recipe.snapshot());
    assert!(
        report.is_complete(),
        "explicit synthetic standard inputs scan completely: {:?}",
        report.issues()
    );
    assert_eq!(
        report.value().get("42"),
        Some("6.0.0"),
        "later prefixes override native and sorted user/current-log candidates"
    );
    for (change, expected) in [
        ("10", "4.0.1"),
        ("11", "4.0.0"),
        ("20", "5.0.1"),
        ("21", "5.0.2"),
    ] {
        assert_eq!(
            report.value().get(change),
            Some(expected),
            "native/current, native/old-only, Wine/current and later-user facts all remain observable"
        );
    }
    assert_eq!(
        report.value().get("420"),
        None,
        "change matching remains exact"
    );
    Ok(())
}

#[test]
fn an_explicit_standard_log_override_is_exclusive_and_does_not_construct_the_recipe() -> TestResult
{
    let temporary = tempfile::tempdir()?;
    let recipe = sources(temporary.path())?;
    let override_folder = temporary.path().join("override");
    fs::create_dir_all(&override_folder)?;
    fs::write(override_folder.join("log.log"), support::patch("9.0.0"))?;
    let constructed = Cell::new(false);
    let report = super::standard_sources(Some(&override_folder), || {
        constructed.set(true);
        recipe.snapshot()
    });
    assert!(
        report.is_complete(),
        "the explicit override is a complete independent source"
    );
    assert_eq!(
        report.value().get("42"),
        Some("9.0.0"),
        "only the override contributes patches"
    );
    assert_eq!(
        report.value().get("10"),
        None,
        "native-only facts cannot leak into an override"
    );
    assert_eq!(
        report.value().get("20"),
        None,
        "Wine-only facts cannot leak into an override"
    );
    assert!(
        !constructed.get(),
        "exclusive override never discovers standard/native/Wine inputs"
    );
    Ok(())
}
