// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{
    Requests, about, archive_key, assembly_export, assembly_export_outcome, catalog_outcome,
    character_export, character_export_setup, extraction, extraction_outcome, licence,
    model_export, model_export_outcome, query, record_export, record_export_outcome, sounds_export,
    sounds_outcome,
};
use crate::Model;

pub(super) fn show(ui: &egui::Ui, model: &mut Model, requests: &mut Requests) {
    extraction::setup(ui, model, requests);
    extraction_outcome::show(ui, model);
    record_export::setup(ui, model, requests);
    record_export_outcome::show(ui, model);
    model_export::setup(ui, model, requests);
    model_export_outcome::show(ui, model);
    assembly_export::setup(ui, model, requests);
    assembly_export_outcome::show(ui, model);
    character_export_setup::setup(ui, model, requests);
    character_export::show(ui, model);
    catalog_outcome::show(ui, model);
    sounds_export::setup(ui, model, requests);
    sounds_outcome::show(ui, model);
    licence::show(ui, model);
    about::show(ui, model);
    archive_key::show(ui, model);
    query::saved_dialog(ui, model, requests);
}
