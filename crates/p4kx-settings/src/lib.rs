// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The settings file of P4KX, a TOML file in the platform config directory, and the
//! Archive key the user stores in the same folder.

mod assembly_export;
mod key_choice;
mod key_store;
mod key_text;
mod locations;
mod model_export;
mod profile;
mod queries;
mod recent;
mod settings;
mod ui_render;

pub use assembly_export::AssemblyExportSettings;
pub use key_choice::{ChosenKey, KeyChoiceError, KeySource, choose_key};
pub use key_store::{KEY_VARIABLE, KeyStore, KeyStoreError};
pub use key_text::KeyText;
pub use locations::{FolderKind, XdgEnvironment, base_folder, cache_folder, data_folder};
pub use model_export::{ModelExportSettings, ModelMaterials};
pub use profile::{ProfileMigration, migrate_profile, profile_folder, profile_folder_in};
pub use queries::{NameProblem, SavedQueries, SavedQuery, SavedQueryEdit};
pub use recent::RecentArchives;
pub use settings::{
    ExtractionAudio, ExtractionTextures, MAX_SETTINGS_SIZE, ManualChannels, Settings,
    SettingsError, WindowGeometry, default_settings_file,
};
pub use ui_render::{
    MAX_VARIABLES_BYTES, UiRenderBackground, UiRenderFormat, UiRenderPower, UiRenderSettings,
    UiRenderSize, bound_variables,
};
