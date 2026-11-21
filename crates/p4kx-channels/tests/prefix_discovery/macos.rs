// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use p4kx_channels::{DiscoveryIssueKind, DiscoveryLimits, WineEnvironment};

use super::fake;

#[test]
fn macos_prefix_priority_excludes_linux_platform_defaults() {
    let home = fake::root();
    let environment = WineEnvironment::macos(
        Some(home.clone()),
        Some(home.join("environment").into_os_string()),
    )
    .with_wine_prefixes(vec![home.join("configured"), "relative".into()])
    .with_crossover_bottle_roots(vec![home.join("custom-bottles")])
    .with_compat_data(Some(home.join("proton").into_os_string()))
    .with_data_home(Some(home.join("xdg")))
    .with_steam_roots(vec![home.join("linux-steam")]);
    let seeds = environment.seeds();
    assert_eq!(
        seeds.direct_prefixes(),
        [
            home.join("environment"),
            home.join("configured"),
            home.join(".wine")
        ],
        "explicit and conventional Wine prefixes preserve their priority"
    );
    let standard = home.join("Library/Application Support/CrossOver/Bottles");
    let configured = home.join("custom-bottles");
    let mut roots = vec![configured, standard];
    let shared = std::path::Path::new("/Library/Application Support/CrossOver/Bottles");
    if shared.is_absolute() {
        roots.push(shared.to_path_buf());
    }
    assert_eq!(
        seeds.crossover_bottle_roots(),
        roots,
        "configured, user and shared roots"
    );
    assert!(
        seeds.steam_roots().is_empty()
            && seeds.steam_libraries().is_empty()
            && seeds.bottles_roots().is_empty()
            && seeds.games_roots().is_empty(),
        "Linux Steam, Proton, Bottles and Lutris defaults do not become Mac probes"
    );
}

#[test]
fn macos_bottle_discovery_keeps_first_alias_and_usable_partial_results() {
    let home = fake::root();
    let explicit = home.join("explicit-wine");
    let configured = home.join("custom");
    let denied = home.join("denied");
    let bottle = configured.join("Bottle");
    let unrelated = configured.join("Not a bottle");
    let standard = home.join("Library/Application Support/CrossOver/Bottles/Standard");
    let alias = home.join("Library/Application Support/CrossOver/Bottles/Z alias");
    let mut fs = fake::FakeFs::default();
    fs.directory(&explicit.join("drive_c"));
    fs.directory(&bottle.join("DrIvE_C"));
    fs.directory(&unrelated);
    fs.directory(&standard.join("drive_c"));
    fs.directory(&alias.join("drive_c"));
    fs.alias(&alias, &bottle);
    fs.fail(&denied, io::ErrorKind::PermissionDenied);

    let environment = WineEnvironment::macos(Some(home), Some(explicit.clone().into_os_string()))
        .with_crossover_bottle_roots(vec![configured, denied.clone()]);
    let report = environment.discover_with(&fs, DiscoveryLimits::default());
    assert_eq!(
        report.value().as_slice(),
        [explicit, bottle, standard],
        "the earlier physical prefix wins, invalid child is excluded and later standard source survives"
    );
    assert!(
        report.issues().iter().any(|issue| issue.path() == denied
            && matches!(issue.kind(), DiscoveryIssueKind::Io(error)
                    if error.kind() == io::ErrorKind::PermissionDenied)),
        "failed root remains explicit even when usable prefixes exist"
    );
}
