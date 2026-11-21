// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io::Cursor;
use std::path::{Path, PathBuf};

use crate::detect::SearchPaths;
use crate::discovery_context::DiscoveryContext;
use crate::discovery_limits::{DiscoveryLimit, DiscoveryLimits};
use crate::discovery_report::DiscoveryIssueKind;
use crate::launcher_log::Wanted;
use crate::launcher_snapshot::LauncherSnapshot;
use crate::prefix_fs::NativePrefixFileSystem;
use crate::search_recipe::SearchRecipe;
use crate::wine::WineEnvironment;

use crate::launcher_test_support as support;
use support::TestResult;

#[test]
fn parsed_payload_bytes_use_exact_boundaries_and_retain_only_admitted_prior_facts() -> TestResult {
    let bytes = b"(SC LIVE 5.0.0-live.42)\n(SC LIVE 5.0.1-live.43)\n";
    for (maximum, first, second, complete) in [
        (6, None, None, false),
        (7, Some("5.0.0"), None, false),
        (14, Some("5.0.0"), Some("5.0.1"), true),
    ] {
        let limits = DiscoveryLimits {
            fact_bytes: maximum,
            ..DiscoveryLimits::default()
        };
        let fs = NativePrefixFileSystem;
        let mut context = DiscoveryContext::new(&fs, limits);
        let parsed = super::parse_input(
            &mut context,
            Path::new("synthetic/log.log"),
            Cursor::new(bytes),
            u64::try_from(bytes.len())?,
            Wanted::PATCHES,
        );
        assert_eq!(
            parsed.log.patches.get("42"),
            first,
            "first fact obeys exact payload admission"
        );
        assert_eq!(
            parsed.log.patches.get("43"),
            second,
            "later facts cannot exceed the aggregate payload"
        );
        assert_eq!(
            parsed.complete, complete,
            "a payload cut cannot become a complete scan"
        );
        assert_eq!(
            context.issues.iter().any(|issue| matches!(
                issue.kind(),
                DiscoveryIssueKind::Limit(DiscoveryLimit::FactBytes)
            )),
            !complete,
            "payload cuts expose their own typed resource rather than silent truncation"
        );
    }
    Ok(())
}

#[test]
fn physical_alias_interpretations_share_capture_but_have_an_aggregate_replayed_path_budget()
-> TestResult {
    let temporary = tempfile::tempdir()?;
    let first = temporary.path().join("Alpha");
    let second = temporary.path().join("Bravo");
    let a = support::launcher(&first, "Pilot")?.join("log.log");
    let b = support::launcher(&second, "Pilot")?.join("log.log");
    let left = first.join("drive_c/SharedGame");
    let right = second.join("drive_c/SharedGame");
    fs::create_dir_all(&left)?;
    fs::create_dir_all(&right)?;
    fs::write(
        &a,
        "{ \"message\": \"Installing Star Citizen LIVE at C:\\\\SharedGame (force DP false)\" }\n",
    )?;
    fs::hard_link(&a, &b)?;
    let one = left.as_os_str().as_encoded_bytes().len();
    assert_eq!(
        right.as_os_str().as_encoded_bytes().len(),
        one,
        "both contexts have equal payload lengths"
    );
    for (maximum, expected, complete) in [
        (one, vec![left.clone()], false),
        (one * 2, vec![left, right], true),
    ] {
        let limits = DiscoveryLimits {
            replayed_bytes: maximum,
            ..DiscoveryLimits::default()
        };
        let recipe = SearchRecipe::with_environment(
            SearchPaths {
                wine_prefixes: vec![first.clone(), second.clone()],
                ..SearchPaths::default()
            },
            WineEnvironment::new(None, None),
            limits,
        );
        let snapshot = recipe.snapshot();
        assert_replayed_libraries(&snapshot, &expected, complete);
    }
    Ok(())
}

fn assert_replayed_libraries(snapshot: &LauncherSnapshot, expected: &[PathBuf], complete: bool) {
    assert_eq!(
        snapshot.logs.len(),
        1,
        "the aliased file has one physical parser owner"
    );
    assert_eq!(
        snapshot.occurrences.len(),
        2,
        "both original interpretation contexts remain ordered"
    );
    let report = super::scan(snapshot, Wanted::LIBRARIES);
    assert_eq!(
        report.value().libraries.as_slice(),
        expected,
        "only admitted interpreted paths enter the result"
    );
    assert_eq!(
        report.is_complete(),
        complete,
        "the shared replay quota controls cache admission"
    );
    assert_eq!(
        report.issues().iter().any(|issue| matches!(
            issue.kind(),
            DiscoveryIssueKind::Limit(DiscoveryLimit::ReplayedBytes)
        )),
        !complete,
        "a context amplification cut is typed and retried later"
    );
}
