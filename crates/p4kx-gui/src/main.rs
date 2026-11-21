// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! `p4kx-gui`, the P4KX window.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{Result, anyhow};
use p4kx_channels::SearchRecipe;
use p4kx_gui::smoke::Frames;
use p4kx_gui::{
    App, ChannelList, Language, Model, PrerequisiteNotice, ProtectionText, RecordTextChoice, Text,
    Texts, frame, theme, window,
};
use p4kx_settings::{
    KEY_VARIABLE, KeyChoiceError, KeyStore, Settings, choose_key, default_settings_file,
};

/// Gives the model the key of the environment variable or the profile. A variable that holds no
/// valid key is ignored in favour of the stored key, and so is a stored key that cannot be read
/// in favour of none; the dialog says so in both cases.
fn set_up_key(model: &mut Model) {
    let key_store = std::env::var_os("P4KX_KEY_FILE")
        .filter(|file| !file.is_empty())
        .map_or_else(KeyStore::in_profile, |file| Some(KeyStore::new(file)));
    let variable = std::env::var(KEY_VARIABLE).ok();
    let (chosen, variable_invalid) = match choose_key(None, variable.as_deref(), key_store.as_ref())
    {
        Ok(chosen) => (chosen, false),
        Err(KeyChoiceError::Environment(_)) => (
            choose_key(None, None, key_store.as_ref()).ok().flatten(),
            true,
        ),
        Err(KeyChoiceError::Flag(_) | KeyChoiceError::Profile(_)) => (None, false),
    };
    model.set_key_store(key_store);
    model.set_key_variable_invalid(variable_invalid);
    model.set_archive_key(chosen.map(|chosen| chosen.key));
}

fn main() -> Result<()> {
    if informational_argument()? {
        return Ok(());
    }
    let runtime = check_runtime()?;
    let settings_file = default_settings_file();
    let mut settings = settings_file
        .as_deref()
        .and_then(|file| Settings::load(file).ok())
        .unwrap_or_default();
    let wine = std::mem::take(&mut settings.wine);
    let recipe = Arc::new(
        SearchRecipe::standard().with_wine_locations(wine.prefixes, wine.crossover_bottle_roots),
    );
    p4kx_extraction::configure_library_discovery(Arc::clone(&recipe));
    let protection_issues = p4kx_extraction::protect_libraries_report();
    let channels = ChannelList::from_shared_recipe(recipe, settings.channels.clone())
        .with_startup_issues(protection_issues);
    let mut model = configured_model(&settings, channels);
    set_up_key(&mut model);
    let title = model.texts().get(Text::AppTitle);
    let options = window::native_options(window::viewport(settings.window, &title));
    let archive = std::env::args_os().nth(1).map(PathBuf::from);
    let smoke = Frames::from_environment();
    eframe::run_native(
        &title,
        options,
        Box::new(move |creation| {
            #[cfg(target_os = "macos")]
            if smoke.is_some() {
                require_metal(creation)?;
            }
            theme::apply(&creation.egui_ctx);
            frame::tint(creation);
            let mut model = model;
            if smoke.is_none() {
                let ctx = creation.egui_ctx.clone();
                model.check_for_update(move || ctx.request_repaint());
            }
            if let Some(path) = archive {
                let ctx = creation.egui_ctx.clone();
                model.open(path, move || ctx.request_repaint());
            }
            let app =
                App::new(model, settings_file, settings.window).with_runtime_environment(runtime);
            Ok(Box::new(match smoke {
                Some(frames) => app.with_smoke(frames),
                None => app,
            }))
        }),
    )
    .map_err(|error| anyhow!("cannot show the window: {error}"))
}

#[cfg(target_os = "macos")]
fn require_metal(creation: &eframe::CreationContext<'_>) -> std::io::Result<()> {
    use eframe::egui_wgpu::wgpu::Backend;

    let backend = creation
        .wgpu_render_state
        .as_ref()
        .map(|state| state.adapter.get_info().backend);
    if backend != Some(Backend::Metal) {
        return Err(std::io::Error::other(format!(
            "the macOS GUI smoke requires an active Metal adapter, got {backend:?}"
        )));
    }
    Ok(())
}

fn configured_model(settings: &Settings, channels: ChannelList) -> Model {
    let mut model = Model::new(Language::resolve(settings.language.as_deref()));
    model.set_channels(channels);
    model.set_recent(settings.recent.clone());
    model.set_extraction_folder(settings.extraction_folder.clone());
    model.set_extraction_conversion(settings.extraction_conversion);
    model.set_extraction_textures(settings.extraction_textures);
    model.set_extraction_audio(settings.extraction_audio);
    model.set_model_export_settings(settings.model_export.clone());
    model.set_assembly_export_settings(settings.assembly_export.clone());
    model.set_ui_render_settings(&settings.ui);
    model.set_update_check(settings.update_check);
    model.watch_game(true);
    model.query_mut().set_saved(settings.queries.clone());
    model.set_record_text_choice(
        settings
            .record_language
            .as_deref()
            .and_then(RecordTextChoice::from_code),
    );
    model
}

fn informational_argument() -> Result<bool> {
    use std::io::Write as _;
    let argument = std::env::args_os().nth(1);
    let text = match argument.as_deref().and_then(std::ffi::OsStr::to_str) {
        Some("--version" | "-V") => format!("p4kx-gui {}", env!("CARGO_PKG_VERSION")),
        Some("--help" | "-h") => {
            Texts::new(Language::resolve(None)).get(ProtectionText::CommandHelp)
        }
        _ => return Ok(false),
    };
    writeln!(std::io::stdout().lock(), "{text}")?;
    Ok(true)
}

fn check_runtime() -> Result<p4kx_extraction::RuntimeEnvironment> {
    match p4kx_extraction::check_runtime_environment() {
        Ok(runtime) => Ok(runtime),
        Err(error) => {
            if Frames::from_environment().is_none() {
                show_prerequisite()?;
            }
            Err(error.into())
        }
    }
}

fn show_prerequisite() -> Result<()> {
    let language = Language::resolve(None);
    let title = Texts::new(language).get(Text::AppTitle);
    let options = window::native_options(window::viewport(None, &title));
    eframe::run_native(
        &title,
        options,
        Box::new(move |creation| {
            theme::apply(&creation.egui_ctx);
            frame::tint(creation);
            Ok(Box::new(PrerequisiteNotice::new(language)))
        }),
    )
    .map_err(|error| anyhow!("cannot show the prerequisite notice: {error}"))
}
