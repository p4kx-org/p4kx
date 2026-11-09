// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;

use p4kx_channels::{DiscoveryIssue, DiscoveryIssueKind, WineEnvironment};

pub(crate) type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

pub(crate) fn issues() -> TestResult<Vec<DiscoveryIssue>> {
    let temporary = tempfile::tempdir()?;
    let steam = temporary.path().join("steam");
    fs::create_dir_all(steam.join("steamapps"))?;
    fs::write(
        steam.join("steamapps/libraryfolders.vdf"),
        "libraryfolders { ",
    )?;
    let environment = WineEnvironment::new(None, None).with_steam_roots(vec![steam]);
    let (_, issues) = environment.discover().into_parts();
    assert!(
        issues
            .iter()
            .any(|issue| matches!(issue.kind(), DiscoveryIssueKind::MalformedConfig)),
        "the worker transport receives a real typed synthetic source failure"
    );
    Ok(issues)
}
