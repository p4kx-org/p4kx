// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use egui::{Align, Layout, Ui};
use p4kx_settings::ModelMaterials;

use super::assembly_export_picks::picks;
use super::extraction::{foot, group_label, path_field, refusal_notice};
use super::{Dialog, Requests, regions};
use crate::theme::desktop::MODAL_GROUP_GAP;
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, FS_XS, Weight};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames::{self, Tone};
use crate::widgets::placement;
use crate::widgets::text::plain;
use crate::widgets::{tab, toggle};
use crate::{
    ASSEMBLY_LODS, ASSEMBLY_MATERIALS, AssemblyExportForm, AssemblyExportSetup,
    AssemblyExportText as T, ExtractionText, MODEL_TEXTURE_MIPS, Model, PackageProblem, Text,
    Texts,
};

enum Action {
    Submit,
    Close,
    ChooseFile,
    ChooseFolder,
}

/// Shows the export setup of an Entity or Interior container while it is open and applies what the user chose.
pub(super) fn setup(ui: &Ui, model: &mut Model, requests: &mut Requests) {
    let Some((setup, texts)) = model.assembly_export_setup_with_texts() else {
        return;
    };
    let title = texts.get(if setup.is_container() {
        T::SetupTitleInterior
    } else {
        T::SetupTitle
    });
    let close_dialog = texts.get(Text::CloseDialog);
    let id = frames::modal_id("assembly-export-setup");
    let shown = placement::hanging_modal(ui.ctx(), id, Tone::Normal).show(ui.ctx(), |ui| {
        ui.set_width(frames::MODAL_WIDE_BODY_WIDTH);
        let closed = frames::head(ui, (&title, Tone::Normal), true, Some(&close_dialog));
        let room = placement::body_room(ui);
        let body = frames::scrolling_body_in(ui, ("assembly-export-body", room), |ui| {
            choices(ui, texts, setup)
        });
        regions::record(ui.ctx(), |regions| {
            regions.scrolls = body.content_size.y > body.inner_rect.height() + 0.5;
        });
        let action = body.inner;
        let pressed = foot(
            ui,
            texts,
            (T::Start, setup.is_ready()),
            ExtractionText::Close,
        );
        let chosen = match pressed {
            Some(true) => Some(Action::Submit),
            Some(false) => Some(Action::Close),
            None => None,
        };
        if closed {
            Some(Action::Close)
        } else {
            action.or(chosen)
        }
    });
    frames::modal_decoration(
        ui.ctx(),
        shown.response.layer_id,
        shown.response.rect,
        Tone::Normal,
    );
    regions::record(ui.ctx(), |regions| {
        regions.dialogs.push(shown.response.rect);
    });
    let close = shown.should_close();
    match shown.inner {
        Some(Action::Submit) => {
            let ctx = ui.ctx().clone();
            requests.settings_changed |=
                model.submit_assembly_export(move || ctx.request_repaint());
        }
        Some(Action::Close) => model.close_assembly_export_setup(),
        Some(Action::ChooseFile) => requests.dialog = Some(Dialog::AssemblyFile),
        Some(Action::ChooseFolder) => requests.dialog = Some(Dialog::AssemblyPackage),
        None if close => model.close_assembly_export_setup(),
        None => {}
    }
}

fn choices(ui: &mut Ui, texts: &Texts, setup: &mut AssemblyExportSetup) -> Option<Action> {
    let subject = if setup.is_container() {
        T::Container
    } else {
        T::Entity
    };
    let subject = texts.format(subject, &[("name", setup.name().to_owned().into())]);
    ui.label(plain(subject, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT));
    if !setup.is_container() {
        loadout_line(ui, texts, setup);
        ui.add_space(MODAL_GROUP_GAP);
        picks(ui, texts, setup);
    }
    ui.add_space(MODAL_GROUP_GAP);
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| form(ui, texts, setup));
        ui.add_space(MODAL_GROUP_GAP);
        ui.vertical(|ui| lod(ui, texts, setup));
    });
    if setup.form() == AssemblyExportForm::Package {
        let note = plain(texts.get(T::FormPackageNote), FS_XS, Weight::Regular);
        ui.add(egui::Label::new(note.color(palette::GRAY_2_TEXT)).wrap());
    }
    materials(ui, texts, setup);
    if setup.has_interior() {
        ui.add_space(MODAL_GROUP_GAP);
        group_label(ui, &texts.get(T::InteriorHeading));
        let mut interior = setup.interior();
        if toggle::checkbox(ui, &mut interior, &texts.get(T::Interior)).changed() {
            setup.set_interior(interior);
        }
    }
    ui.add_space(MODAL_GROUP_GAP);
    let action = if setup.form() == AssemblyExportForm::Package {
        let texts_of = (T::FolderHeading, T::ChooseFolder, T::FolderNone);
        let pressed = target_row(ui, texts, texts_of, setup.package_folder());
        package_notice(ui, texts, setup);
        pressed.map(|()| Action::ChooseFolder)
    } else {
        let texts_of = (T::FileHeading, T::ChooseFile, T::FileNone);
        target_row(ui, texts, texts_of, setup.file()).map(|()| Action::ChooseFile)
    };
    refusal_notice(ui, texts, setup.refusal());
    action
}

pub(super) fn target_row(
    ui: &mut Ui,
    texts: &Texts,
    (heading, choose, none): (T, T, T),
    chosen: Option<&Path>,
) -> Option<()> {
    let mut pressed = None;
    group_label(ui, &texts.get(heading));
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let choose = texts.get(choose);
            if button::button(ui, Kind::Ghost, Size::Normal, &choose, true).clicked() {
                pressed = Some(());
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                path_field(ui, texts.get(none), chosen);
            });
        });
    });
    pressed
}

fn lod(ui: &mut Ui, texts: &Texts, setup: &mut AssemblyExportSetup) {
    group_label(ui, &texts.get(T::LodHeading));
    let labels = ASSEMBLY_LODS.map(|level| texts.format(T::Lod, &[("level", level.into())]));
    let active = ASSEMBLY_LODS.iter().position(|level| *level == setup.lod());
    let clicked = ui.horizontal(|ui| tab::segments(ui, &labels, active)).inner;
    if let Some(level) = clicked.and_then(|at| ASSEMBLY_LODS.get(at)) {
        setup.set_lod(*level);
    }
}

fn form(ui: &mut Ui, texts: &Texts, setup: &mut AssemblyExportSetup) {
    if let Some(chosen) = form_choice(ui, texts, setup.form()) {
        setup.set_form(chosen);
    }
}

/// Shows the choice between the GLB file and the Blender package; returns the form clicked.
pub(super) fn form_choice(
    ui: &mut Ui,
    texts: &Texts,
    current: AssemblyExportForm,
) -> Option<AssemblyExportForm> {
    group_label(ui, &texts.get(T::FormHeading));
    let labels = [texts.get(T::FormGlb), texts.get(T::FormPackage)];
    let active = Some(usize::from(current == AssemblyExportForm::Package));
    let clicked = ui.horizontal(|ui| tab::segments(ui, &labels, active)).inner;
    match clicked {
        Some(0) => Some(AssemblyExportForm::Glb),
        Some(1) => Some(AssemblyExportForm::Package),
        _ => None,
    }
}

fn package_notice(ui: &mut Ui, texts: &Texts, setup: &AssemblyExportSetup) {
    folder_notice(
        ui,
        texts,
        (setup.package_problem(), setup.package_replaces()),
    );
}

/// Tells what is wrong with the chosen package folder, or that it replaces an earlier package.
pub(super) fn folder_notice(
    ui: &mut Ui,
    texts: &Texts,
    (problem, replaces): (Option<&PackageProblem>, bool),
) {
    match problem {
        Some(PackageProblem::NotAPackage) => {
            frames::danger_alert(ui, |ui| {
                let text = plain(texts.get(T::PackageNotAPackage), FS_SM, Weight::Regular);
                ui.add(egui::Label::new(text.color(palette::GRAY_1)).wrap());
            });
        }
        Some(PackageProblem::NoFolderName) => {
            frames::danger_alert(ui, |ui| {
                let text = plain(texts.get(T::PackageNoName), FS_SM, Weight::Regular);
                ui.add(egui::Label::new(text.color(palette::GRAY_1)).wrap());
            });
        }
        None if replaces => {
            let text = plain(texts.get(T::PackageReplaces), FS_SM, Weight::Regular);
            ui.add(egui::Label::new(text.color(palette::WARNING)).wrap());
        }
        Some(PackageProblem::Installation(_)) | None => {}
    }
}

fn loadout_line(ui: &mut Ui, texts: &Texts, setup: &AssemblyExportSetup) {
    let (excluded, replaced) = (
        setup.changes().exclusions().len(),
        setup.changes().replacements().len(),
    );
    let changes = if excluded + replaced == 0 {
        texts.get(T::ChangesNone)
    } else {
        texts.format(
            T::Changes,
            &[
                ("excluded", excluded.to_string().into()),
                ("replaced", replaced.to_string().into()),
            ],
        )
    };
    ui.label(plain(changes, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT));
}

fn materials(ui: &mut Ui, texts: &Texts, setup: &mut AssemblyExportSetup) {
    let package = setup.form() == AssemblyExportForm::Package;
    if !package {
        ui.add_space(MODAL_GROUP_GAP);
        group_label(ui, &texts.get(T::MaterialsHeading));
        for materials in ASSEMBLY_MATERIALS {
            let label = texts.get(match materials {
                ModelMaterials::Colours => T::MaterialsColours,
                ModelMaterials::Textures => T::MaterialsTextures,
            });
            if toggle::radio(ui, setup.materials() == materials, &label, true).clicked() {
                setup.set_materials(materials);
            }
        }
    }
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(T::TextureMipHeading));
    let mips = package || setup.materials() == ModelMaterials::Textures;
    let labels = MODEL_TEXTURE_MIPS.map(|mip| {
        texts.format(
            T::TextureMip,
            &[("level", mip.into()), ("divisor", (1_u32 << mip).into())],
        )
    });
    let active = MODEL_TEXTURE_MIPS
        .iter()
        .position(|mip| *mip == setup.texture_mip());
    let chosen = ui
        .add_enabled_ui(mips, |ui| {
            ui.horizontal(|ui| tab::segments(ui, &labels, active)).inner
        })
        .inner;
    if let Some(mip) = chosen.and_then(|at| MODEL_TEXTURE_MIPS.get(at)) {
        setup.set_texture_mip(*mip);
    }
    ui.add_space(MODAL_GROUP_GAP);
    let mut colours = setup.vertex_colours();
    if toggle::checkbox(ui, &mut colours, &texts.get(T::VertexColours)).changed() {
        setup.set_vertex_colours(colours);
    }
}
