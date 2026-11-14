// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use egui_kittest::Harness;
use egui_kittest::kittest::Queryable;
use egui_kittest::wgpu::WgpuTestRenderer;
use p4kx_channels::{
    DiscoveryIssueKind, DiscoveryLimits, SearchPaths, SearchRecipe, WineEnvironment,
};
use p4kx_gui::{ChannelList, Language, Model, Text, ui};
use p4kx_settings::ManualChannels;

use crate::rendering;
use crate::waiting::{RunQuiet, wait_polling};

type TestResult = Result<(), Box<dyn Error>>;

fn source(root: &Path) -> Result<(PathBuf, SearchRecipe), Box<dyn Error>> {
    let steam = root.join("steam");
    let config = steam.join("steamapps/libraryfolders.vdf");
    fs::create_dir_all(config.parent().ok_or("config parent")?)?;
    fs::write(&config, "libraryfolders { 1 { path")?;
    let environment = WineEnvironment::new(None, None).with_steam_roots(vec![steam]);
    Ok((
        config,
        SearchRecipe::with_environment(
            SearchPaths::default(),
            environment,
            DiscoveryLimits::default(),
        ),
    ))
}

fn manual(root: &Path) -> Result<ManualChannels, Box<dyn Error>> {
    let folder = root.join("MANUAL");
    fs::create_dir_all(&folder)?;
    let archive = folder.join("Data.p4k");
    fs::write(&archive, b"")?;
    Ok(ManualChannels {
        folders: Vec::new(),
        archives: vec![archive],
    })
}

#[test]
fn incomplete_discovery_warns_in_both_languages_and_keeps_the_manual_channel() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let (_, recipe) = source(temporary.path())?;
    let manual = manual(temporary.path())?;
    for language in [Language::BritishEnglish, Language::German] {
        let list = ChannelList::from_recipe(recipe.clone(), manual.clone());
        assert!(
            list.discovery_issues()
                .iter()
                .any(|issue| matches!(issue.kind(), DiscoveryIssueKind::MalformedConfig)),
            "the model retains the real typed source failure"
        );
        let mut model = Model::new(language);
        let caption = model.texts().get(Text::ChannelsDiscoveryIncomplete);
        model.set_channels(list);
        let mut harness = Harness::builder()
            .with_context_setup(p4kx_gui::theme::apply)
            .with_size([1200.0, 800.0])
            .build_ui_state(
                |ui, model| {
                    ui::show(ui, model);
                },
                model,
            );
        harness.run_quiet();
        assert!(
            harness.query_by_label(&caption).is_some(),
            "the localized warning reaches the UI"
        );
        assert!(
            harness.query_by_label("MANUAL").is_some(),
            "the explicit manual fallback remains usable"
        );
    }
    Ok(())
}

#[test]
fn a_manual_refresh_retries_the_same_recipe_and_clears_recovered_discovery_errors() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let (config, recipe) = source(temporary.path())?;
    let manual = manual(temporary.path())?;
    let archive = manual.archives.first().ok_or("manual Archive")?.clone();
    let mut model = Model::new(Language::BritishEnglish);
    model.set_channels(ChannelList::from_recipe(recipe, manual));
    assert!(
        !model.channels().discovery_issues().is_empty(),
        "the initial incomplete source is visible"
    );
    fs::write(config, "libraryfolders { }")?;
    assert!(
        model.add_manual(temporary.path().join("extra-folder")),
        "a new manual input queues discovery"
    );
    assert!(
        !model.channels().discovery_issues().is_empty(),
        "registration does not scan synchronously"
    );
    wait_polling(
        &mut model,
        "the Channel refresh",
        |model| {
            model.poll();
        },
        |model| model.channels().discovery_issues().is_empty(),
    )?;
    assert!(
        model
            .channels()
            .detected()
            .iter()
            .any(|channel| channel.archive() == archive),
        "recovery preserves the already registered manual Archive"
    );
    Ok(())
}

#[test]
fn discovery_warnings_render_in_both_languages_at_minimum_and_default_sizes() -> TestResult {
    let temporary = tempfile::tempdir()?;
    let (_, recipe) = source(temporary.path())?;
    let manual = manual(temporary.path())?;
    for (language, suffix) in [
        (Language::BritishEnglish, "en-GB"),
        (Language::German, "de-DE"),
    ] {
        for (size, name) in [([640.0, 400.0], "minimum"), ([1280.0, 800.0], "default")] {
            let Some(state) = rendering::render_state() else {
                return Ok(());
            };
            let mut model = Model::new(language);
            let caption = model.texts().get(Text::ChannelsDiscoveryIncomplete);
            let sidebar = model.texts().get(Text::SidebarShow);
            model.set_channels(ChannelList::from_recipe(recipe.clone(), manual.clone()));
            let mut harness = Harness::builder()
                .with_context_setup(p4kx_gui::theme::apply)
                .with_size(size)
                .renderer(WgpuTestRenderer::from_render_state(state))
                .build_ui_state(
                    |ui, model| {
                        ui::show(ui, model);
                    },
                    model,
                );
            harness.run_steps(3);
            if let Some(toggle) = harness.query_by_label(&sidebar) {
                toggle.click();
                harness.run_steps(3);
            }
            let image = harness.render()?;
            rendering::save(
                &format!("channel-discovery-{suffix}-{name}.png"),
                image.width(),
                image.height(),
                &image.into_raw(),
            )?;
            assert!(
                harness.query_by_label(&caption).is_some(),
                "the rendered warning is localized in {language:?} at {name} size"
            );
            assert!(
                harness.query_by_label("MANUAL").is_some(),
                "the manual fallback stays available in the rendered window"
            );
        }
    }
    Ok(())
}
