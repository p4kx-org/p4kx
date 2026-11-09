// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::launcher_test_support as support;

use std::fs;
use std::io::{self, Cursor};
use std::path::Path;

use crate::detect::SearchPaths;
use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::DiscoveryLimits;
use crate::discovery_report::DiscoveryIssueKind;
use crate::launcher_log::Wanted;
use crate::prefix_fs::NativePrefixFileSystem;
use crate::search_recipe::SearchRecipe;

use super::parse_input;
use support::TestResult;

#[test]
fn an_opened_log_error_retains_its_prior_patch_and_never_becomes_absence() -> TestResult {
    for kind in [
        io::ErrorKind::PermissionDenied,
        io::ErrorKind::UnexpectedEof,
        io::ErrorKind::NotFound,
    ] {
        let fs = NativePrefixFileSystem;
        let mut context = DiscoveryContext::new(&fs, DiscoveryLimits::default());
        let bytes = support::patch("5.0.0").into_bytes();
        let length = u64::try_from(bytes.len())? + 1;
        let input = support::FailingInput::new(bytes, kind);
        let parsed = parse_input(
            &mut context,
            Path::new("synthetic/log.log"),
            input,
            length,
            Wanted::PATCHES,
        );
        assert_eq!(
            parsed.log.patches.get("42"),
            Some("5.0.0"),
            "a complete earlier line remains usable"
        );
        assert!(
            !parsed.complete,
            "an actual opened-source read failure forbids cache admission"
        );
        assert!(
            context.issues.iter().any(|issue| matches!(issue.kind(),
            DiscoveryIssueKind::Io(error) if error.kind() == kind)),
            "the original opened-source error remains typed, including NotFound"
        );
    }
    Ok(())
}

#[test]
fn parse_admission_keeps_prior_facts_available_for_the_separate_replay_budget() -> TestResult {
    let fs = NativePrefixFileSystem;
    let limits = DiscoveryLimits {
        log_facts: 1,
        ..DiscoveryLimits::default()
    };
    let mut context = DiscoveryContext::new(&fs, limits);
    let bytes = b"(SC LIVE 5.0.0-live.42)\n(SC LIVE 5.0.1-live.43)\n".to_vec();
    let length = u64::try_from(bytes.len())?;
    let parsed = parse_input(
        &mut context,
        Path::new("synthetic/log.log"),
        Cursor::new(bytes),
        length,
        Wanted::PATCHES,
    );
    assert_eq!(
        parsed.log.patches.get("42"),
        Some("5.0.0"),
        "the admitted first fact survives"
    );
    assert_eq!(
        parsed.log.patches.get("43"),
        None,
        "the unadmitted second fact is not retained"
    );
    assert!(
        !parsed.complete,
        "the capture cut is an incomplete source read"
    );
    assert!(
        context.budget.facts_exhausted(),
        "the actual parser used the configured capture admission"
    );
    assert!(
        !context.budget.replay_exhausted(),
        "captured facts still have their independent replay budget"
    );
    Ok(())
}

#[test]
fn physical_aliases_parse_once_but_replay_a_c_a_in_original_context_order() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let native = temporary.path().join("native");
    fs::create_dir_all(native.join("logs"))?;
    let a = native.join("logs/log.log");
    fs::write(&a, support::patch("5.0.0"))?;
    let prefix_c = temporary.path().join("prefix-c");
    let prefix_a = temporary.path().join("prefix-a");
    let c = support::launcher(&prefix_c, "pilot")?;
    let alias = support::launcher(&prefix_a, "pilot")?;
    fs::write(c.join("log.log"), support::patch("5.0.1"))?;
    fs::hard_link(&a, alias.join("log.log"))?;
    let paths = SearchPaths {
        launcher: Some(native),
        wine_prefixes: vec![prefix_c, prefix_a],
        ..SearchPaths::default()
    };
    let snapshot = SearchRecipe::explicit(paths.clone()).snapshot();
    assert_eq!(
        snapshot.logs.len(),
        2,
        "one physical reader exists for each distinct file"
    );
    assert_eq!(
        snapshot.occurrences.len(),
        3,
        "all three genuine interpretation occurrences remain"
    );
    let first = snapshot.read_patches();
    assert!(
        first.is_complete(),
        "the complete alias scan remains cacheable: {:?}",
        first.issues()
    );
    assert_eq!(
        first.value().get("42"),
        Some("5.0.0"),
        "the later A context wins after C"
    );
    fs::remove_file(alias.join("log.log"))?;
    let next = SearchRecipe::explicit(paths).snapshot();
    assert_ne!(
        snapshot.stamp(),
        next.stamp(),
        "alias removal changes topology despite the primary A staying open"
    );
    assert_eq!(
        next.read_patches().value().get("42"),
        Some("5.0.1"),
        "without the later A occurrence C wins"
    );
    Ok(())
}

#[test]
fn native_and_wine_aliases_keep_windows_library_translation_after_physical_dedup() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let native = temporary.path().join("native");
    fs::create_dir_all(native.join("logs"))?;
    let prefix = temporary.path().join("prefix");
    let logs = support::launcher(&prefix, "pilot")?;
    let library = prefix.join("drive_c/SharedGame");
    fs::create_dir_all(&library)?;
    let line = "{\"msg\":\"Installing Star Citizen LIVE at C:\\\\SharedGame (force DP\"}\n";
    let a = native.join("logs/log.log");
    fs::write(&a, line)?;
    fs::hard_link(&a, logs.join("log.log"))?;
    let snapshot = SearchPaths {
        launcher: Some(native),
        wine_prefixes: vec![prefix],
        ..SearchPaths::default()
    }
    .launcher_snapshot();
    assert_eq!(
        snapshot.logs.len(),
        1,
        "the aliased physical log has one read owner"
    );
    let report = snapshot.read_libraries();
    assert!(
        report.is_complete(),
        "both actual contexts are readable: {:?}",
        report.issues()
    );
    assert!(
        report.value().contains(&library),
        "the later Wine context still translates C: into its actual drive"
    );
    Ok(())
}

#[test]
fn the_physical_opened_set_contains_only_fixed_logs_among_native_and_wine_decoys() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let native = temporary.path().join("native");
    let native_logs = native.join("logs");
    fs::create_dir_all(&native_logs)?;
    let prefix = temporary.path().join("prefix");
    let wine_logs = support::launcher(&prefix, "Pilot")?;
    for folder in [&native_logs, &wine_logs] {
        for name in ["log.log", "log.old.log"] {
            fs::write(folder.join(name), support::patch("5.0.0"))?;
        }
        for name in ["loginData.json", "store.json", "user.reg"] {
            fs::write(folder.join(name), b"synthetic decoy")?;
        }
    }
    let snapshot = SearchRecipe::explicit(SearchPaths {
        launcher: Some(native),
        wine_prefixes: vec![prefix],
        ..SearchPaths::default()
    })
    .snapshot();
    assert!(
        snapshot.is_complete(),
        "decoy files are outside the known source inventory"
    );
    assert_eq!(
        snapshot.logs.len(),
        4,
        "only two actual files from each admitted source are opened"
    );
    assert!(
        snapshot
            .logs
            .iter()
            .all(|file| ["log.log", "log.old.log"].contains(&file.name)),
        "every retained physical handle belongs to an allowed fixed log name"
    );
    let patches = snapshot.read_patches();
    assert!(
        patches.is_complete(),
        "only the known regular logs participate in scanning"
    );
    assert_eq!(
        patches.value().get("42"),
        Some("5.0.0"),
        "the same measured parser returns the expected patch"
    );
    Ok(())
}
