// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::{Path, PathBuf};

use p4kx_channels::{
    DiscoveryIssueKind, DiscoveryLimit, DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment,
};

use super::fake;

fn direct() -> (fake::FakeFs, PathBuf, WineEnvironment) {
    let home = fake::root();
    let prefix = home.join("explicit");
    let bottles = home.join("bottles");
    let mut fs = fake::FakeFs::default();
    fs.directory(&prefix);
    fs.directory(&bottles.join("Alpha"));
    fs.directory(&bottles.join("Beta"));
    let environment = WineEnvironment::new(None, Some(prefix.clone().into_os_string()))
        .with_bottles_roots(vec![bottles]);
    (fs, prefix, environment)
}

#[test]
fn an_oversized_directory_is_discarded_instead_of_sampling_arbitrary_entries() {
    let (fs, prefix, environment) = direct();
    let mut limits = DiscoveryLimits::default();
    limits.entries_per_directory = 1;
    let report = environment.discover_with(&fs, limits);
    assert_eq!(
        report.value(),
        &[prefix],
        "the earlier explicit prefix survives the failed family"
    );
    assert!(
        report.issues().iter().any(|issue| matches!(
            issue.kind(),
            DiscoveryIssueKind::Limit(DiscoveryLimit::DirectoryEntries)
        )),
        "the directory cut is explicit"
    );
}

#[test]
fn a_partial_directory_error_does_not_admit_its_successful_prefix_sample() {
    let (mut fs, prefix, environment) = direct();
    let bottles = fake::root().join("bottles");
    fs.entries(
        &bottles,
        vec![
            fake::Entry::Name("Alpha".into()),
            fake::Entry::Failure(io::ErrorKind::PermissionDenied),
        ],
    );
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert_eq!(
        report.value(),
        &[prefix],
        "a failing directory is atomic, not a partial sample"
    );
    assert!(
        report.issues().iter().any(|issue| matches!(issue.kind(),
        DiscoveryIssueKind::Io(error) if error.kind() == io::ErrorKind::PermissionDenied)),
        "the original entry failure is retained"
    );
}

#[test]
fn global_operation_admission_bounds_the_actual_adapter_calls() {
    let (fs, _, environment) = direct();
    let mut limits = DiscoveryLimits::default();
    limits.operations = 1;
    let report = environment.discover_with(&fs, limits);
    assert!(
        !report.is_complete(),
        "an exhausted aggregate budget cannot report complete discovery"
    );
    assert_eq!(
        fs.probes.borrow().len(),
        1,
        "no adapter call happens after admission fails"
    );
    assert!(
        report.issues().iter().any(|issue| matches!(
            issue.kind(),
            DiscoveryIssueKind::Limit(DiscoveryLimit::Operations)
        )),
        "the exhausted resource is typed"
    );
}

#[test]
fn missing_roots_are_absent_but_unreadable_roots_are_not_absence() {
    let root = fake::root().join("unreadable");
    let mut fs = fake::FakeFs::default();
    fs.fail(&root, io::ErrorKind::PermissionDenied);
    let environment = WineEnvironment::new(None, Some(root.clone().into_os_string()));
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        !report.is_complete(),
        "an unreadable explicit candidate is a partial discovery"
    );
    assert!(
        report
            .issues()
            .iter()
            .any(|issue| issue.path() == root.as_path()
                && matches!(issue.kind(),
        DiscoveryIssueKind::Io(error) if error.kind() == io::ErrorKind::PermissionDenied)),
        "path and original error survive"
    );
    let missing = environment.discover_with(&fake::FakeFs::default(), DiscoveryLimits::default());
    assert!(
        missing.is_complete(),
        "ordinary missing paths are not read failures"
    );
}

#[test]
fn explicit_prefix_vectors_are_admitted_before_copying_or_source_iteration() {
    let root = fake::root();
    let paths = SearchPaths {
        wine_prefixes: (0..10_000)
            .map(|index| root.join(format!("prefix-{index}")))
            .collect(),
        manual_folders: vec![root.join("manual")],
        ..SearchPaths::default()
    };
    let fs = fake::FakeFs::default();
    let mut limits = DiscoveryLimits::default();
    limits.roots = 2;
    limits.prefixes = 2;
    limits.operations = 0;
    let snapshot = paths.launcher_snapshot_with(&fs, limits);
    assert_eq!(
        snapshot.paths().wine_prefixes,
        [root.join("prefix-0"), root.join("prefix-1")],
        "only admitted prefixes reach the owned snapshot and source topology"
    );
    assert_eq!(
        snapshot.paths().manual_folders,
        paths.manual_folders,
        "automatic admission preserves the explicit manual fallback"
    );
    assert_eq!(
        snapshot.issues().len(),
        1,
        "later bounded loops do not multiply a cut diagnostic"
    );
    assert!(
        fs.probes.borrow().is_empty(),
        "zero operations admits no filesystem call"
    );
    let recipe = SearchRecipe::with_environment(paths, WineEnvironment::new(None, None), limits);
    let refreshed = recipe.refresh_with(&fs);
    assert_eq!(
        refreshed.value().wine_prefixes.len(),
        2,
        "recipe refresh uses the same admission before copying explicit prefix inputs"
    );
    assert_eq!(
        refreshed.issues().len(),
        1,
        "one cut remains one diagnostic across refresh phases"
    );
    assert_manual_snapshot(&recipe, &root);
}

#[test]
fn configured_environment_roots_stop_before_cloning_the_unbounded_tail() {
    let roots = (0..10_000)
        .map(|index| fake::root().join(format!("steam-{index}")))
        .collect();
    let environment = WineEnvironment::new(None, None).with_steam_roots(roots);
    let fs = fake::FakeFs::default();
    let mut limits = DiscoveryLimits::default();
    limits.operations = 0;
    let report = environment.discover_with(&fs, limits);
    assert_eq!(
        report.issues().len(),
        1,
        "candidate walking stops when aggregate admission fails"
    );
    assert!(
        fs.probes.borrow().is_empty(),
        "bounded source planning performs no forbidden call"
    );
}

fn assert_manual_snapshot(recipe: &SearchRecipe, root: &Path) {
    let folders = [root.join("manual-override")];
    let archives = [root.join("manual-override.p4k")];
    let manual = recipe.snapshot_with_manual(&folders, &archives);
    assert_eq!(
        manual.paths().wine_prefixes.len(),
        2,
        "borrowed manual snapshot retains the pre-copy prefix admission"
    );
    assert_eq!(
        manual.paths().manual_folders,
        folders,
        "manual override is copied only into its final snapshot"
    );
    assert_eq!(
        manual.paths().manual_archives,
        archives,
        "manual Archives retain exact order and identity"
    );
}
