// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;
use std::path::PathBuf;

use p4kx_channels::{DiscoveryIssueKind, DiscoveryLimit, DiscoveryLimits, WineEnvironment};

use super::{TestResult, fake};

fn source(bytes: Vec<u8>) -> (fake::FakeFs, PathBuf, WineEnvironment) {
    let steam = fake::root().join("steam");
    let config = steam.join("steamapps/libraryfolders.vdf");
    let mut fs = fake::FakeFs::default();
    fs.config(&config, bytes);
    (
        fs,
        config,
        WineEnvironment::new(None, None).with_steam_roots(vec![steam]),
    )
}

#[test]
fn confirmed_configuration_not_found_is_partial_and_the_next_discovery_retries() -> TestResult {
    let library = fake::root().join("external");
    let text = format!(
        "libraryfolders {{ 1 {} }}",
        serde_json::to_string(&library)?
    );
    let (mut fs, config, environment) = source(text.into_bytes());
    fs.fail_config(&config, io::ErrorKind::NotFound);
    let failed = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        !failed.is_complete(),
        "an observed file disappearing at open is a partial source"
    );
    assert!(
        failed.issues().iter().any(|issue| issue.path() == config
            && matches!(issue.kind(),
        DiscoveryIssueKind::Io(error) if error.kind() == io::ErrorKind::NotFound)),
        "the confirmed-file race retains its actual typed error"
    );
    let (mut recovered, _, _) = source(
        format!(
            "libraryfolders {{ 1 {} }}",
            serde_json::to_string(&library)?
        )
        .into_bytes(),
    );
    let prefix = super::library(&mut recovered, &library, "1");
    let retried = environment.discover_with(&recovered, DiscoveryLimits::default());
    assert!(
        retried.is_complete(),
        "a new scan retries the failed configuration"
    );
    assert_eq!(
        retried.value(),
        &[prefix],
        "the recovered source contributes its actual prefix"
    );
    Ok(())
}

#[test]
fn malformed_truncated_and_non_utf8_configurations_report_without_partial_roots() {
    for bytes in [
        b"libraryfolders { 1 /outside".to_vec(),
        b"wrong { }".to_vec(),
        vec![0xff],
    ] {
        let (fs, _, environment) = source(bytes);
        let report = environment.discover_with(&fs, DiscoveryLimits::default());
        assert!(
            report.value().is_empty(),
            "no partial unvalidated configuration root escapes"
        );
        assert!(
            report
                .issues()
                .iter()
                .any(|issue| matches!(issue.kind(), DiscoveryIssueKind::MalformedConfig)),
            "malformed syntax and text remain explicit"
        );
    }
}

#[test]
fn configuration_byte_token_depth_and_root_cuts_name_the_exhausted_resource() -> TestResult {
    let text = format!(
        "libraryfolders {{ 1 {{ path {} apps {{ 1 1 }} }} }}",
        serde_json::to_string(&fake::root().join("external"))?
    );
    for resource in [
        DiscoveryLimit::ConfigFileBytes,
        DiscoveryLimit::ConfigBytes,
        DiscoveryLimit::ConfigTokens,
        DiscoveryLimit::ConfigDepth,
        DiscoveryLimit::Roots,
    ] {
        let (fs, _, environment) = source(text.as_bytes().to_vec());
        let mut limits = DiscoveryLimits::default();
        match resource {
            DiscoveryLimit::ConfigFileBytes => limits.config_file_bytes = 8,
            DiscoveryLimit::ConfigBytes => limits.config_bytes = 8,
            DiscoveryLimit::ConfigTokens => limits.config_tokens = 1,
            DiscoveryLimit::ConfigDepth => limits.config_depth = 1,
            DiscoveryLimit::Roots => limits.roots = 1,
            _ => continue,
        }
        let report = environment.discover_with(&fs, limits);
        assert!(
            report.value().is_empty(),
            "a cut never publishes partial configured roots"
        );
        assert!(
            report.issues().iter().any(|issue| matches!(issue.kind(),
            DiscoveryIssueKind::Limit(actual) if *actual == resource)),
            "the cut reports its exact configured resource: {resource:?}"
        );
    }
    Ok(())
}

#[test]
fn commented_escaped_unicode_and_unknown_nested_app_fields_are_parsed() -> TestResult {
    let library = fake::root().join("external ä quoted \" folder");
    let text = format!(
        "// configured library\nlibraryfolders {{ 1 {{ path {} apps {{ 1 {{ x y }} }} }} }}",
        serde_json::to_string(&library)?
    );
    let (mut fs, _, environment) = source(text.into_bytes());
    let prefix = super::library(&mut fs, &library, "42");
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert!(
        report.is_complete(),
        "supported lexical forms remain complete: {:?}",
        report.issues()
    );
    assert_eq!(
        report.value(),
        &[prefix],
        "escaped names are decoded into the actual configured path"
    );
    Ok(())
}
