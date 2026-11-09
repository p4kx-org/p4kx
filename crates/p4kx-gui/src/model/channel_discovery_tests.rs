// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use p4kx_channels::{DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment};
use p4kx_settings::ManualChannels;

use crate::channel_refresh::RefreshFailure;
use crate::discovery_test_support::TestResult;
use crate::{ChannelList, Language, Model, Text, ui};

use super::super::Failure;

const WAIT: Duration = Duration::from_secs(10);

fn malformed_model(root: &Path) -> TestResult<(Model, PathBuf)> {
    let steam = root.join("steam");
    fs::create_dir_all(steam.join("steamapps"))?;
    let config = steam.join("steamapps/libraryfolders.vdf");
    fs::write(&config, "libraryfolders { ")?;
    let recipe = SearchRecipe::with_environment(
        SearchPaths::default(),
        WineEnvironment::new(None, None).with_steam_roots(vec![steam]),
        DiscoveryLimits::default(),
    );
    let mut model = Model::new(Language::BritishEnglish);
    model.set_channels(ChannelList::from_recipe(recipe, ManualChannels::default()));
    Ok((model, config))
}

#[test]
fn manual_registration_keeps_immediate_protection_and_defers_automatic_adoption_until_poll()
-> TestResult {
    let root = tempfile::tempdir()?;
    let (mut model, config) = malformed_model(root.path())?;
    assert!(
        !model.channels().discovery_issues().is_empty(),
        "startup measured the malformed configuration"
    );
    let folder = root.path().join("PTU");
    fs::create_dir_all(folder.join("Bin64"))?;
    let archive = folder.join("Data.p4k");
    fs::write(&archive, b"")?;
    fs::write(config, "libraryfolders { }")?;
    model.failure = Some(Failure {
        path: root.path().join("primary.p4k"),
        reason: Some("primary sentinel".to_owned()),
    });
    let (signal, wake) = mpsc::channel();
    assert!(
        model.add_manual_with_wake(folder.clone(), move || {
            let _delivery = signal.send(());
        }),
        "accepted manual settings are recorded before the worker result"
    );
    assert!(
        model.channels().manual().folders.contains(&folder),
        "the setting is available for immediate persistence"
    );
    assert!(
        p4kx_extraction::ensure_writable(&folder.join("export.bin")).is_err(),
        "the existing immediate manual-protection guarantee remains in force"
    );
    assert!(
        !model.channels().discovery_issues().is_empty(),
        "no synchronous automatic scan/adoption happened in add"
    );
    wake.recv_timeout(WAIT)?;
    assert!(
        !model.poll(),
        "a discovery completion does not impersonate a primary archive opening"
    );
    assert!(
        model.channels().discovery_issues().is_empty(),
        "the current background report repairs source diagnostics"
    );
    assert!(
        model
            .channels()
            .detected()
            .iter()
            .any(|channel| channel.archive() == archive),
        "the worker result contains the newly accepted manual Archive"
    );
    assert_eq!(
        model
            .failure()
            .and_then(|failure| failure.reason.as_deref()),
        Some("primary sentinel"),
        "automatic discovery does not replace the existing primary Archive failure"
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum FailureKind {
    Spawn,
    Stopped,
}

fn failed_model(language: Language, kind: FailureKind) -> Model {
    let mut model = Model::new(language);
    model.failure = Some(Failure {
        path: "primary.p4k".into(),
        reason: Some("primary sentinel".to_owned()),
    });
    let search = model.channels.search();
    model.channel_refresh.request_with(
        search,
        || {},
        |_job| match kind {
            FailureKind::Stopped => Ok(()),
            FailureKind::Spawn => Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "spawn sentinel",
            )),
        },
    );
    model.poll();
    model
}

#[test]
fn current_spawn_and_disconnect_failures_reach_the_existing_localized_warning_surface() {
    for language in [Language::BritishEnglish, Language::German] {
        for kind in [FailureKind::Spawn, FailureKind::Stopped] {
            let model = failed_model(language, kind);
            if matches!(kind, FailureKind::Stopped) {
                assert!(
                    matches!(
                        model.channel_refresh_failure(),
                        Some(RefreshFailure::Stopped)
                    ),
                    "the dropped worker is reported through the real receiver branch"
                );
            } else {
                assert!(
                    matches!(model.channel_refresh_failure(), Some(RefreshFailure::Spawn(error))
                    if error.to_string() == "spawn sentinel"),
                    "the original spawn reason remains available"
                );
            }
            let caption = model.texts().get(Text::ChannelsDiscoveryIncomplete);
            let mut harness = Harness::builder()
                .with_context_setup(crate::theme::apply)
                .with_size([1200.0, 800.0])
                .build_ui_state(
                    |ui, model| {
                        ui::show(ui, model);
                    },
                    model,
                );
            harness.run_steps(3);
            assert!(
                harness.query_by_label(&caption).is_some(),
                "a real worker failure shows the existing localized alert"
            );
            assert_eq!(
                harness
                    .state()
                    .failure()
                    .and_then(|failure| failure.reason.as_deref()),
                Some("primary sentinel"),
                "the discovery alert does not mask the primary Archive failure"
            );
        }
    }
}
