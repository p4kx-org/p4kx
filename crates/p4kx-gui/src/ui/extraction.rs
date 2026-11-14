// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;
use std::time::SystemTime;

use egui::{Align, Align2, Frame, Layout, Margin, TextEdit, Ui, vec2};
use fluent_bundle::FluentValue;
use p4kx_extraction::{ConflictPolicy, WriteProtectionError, default_target_name};
use p4kx_settings::{ExtractionAudio, ExtractionTextures};

use super::{Dialog, Requests, comparison_extraction, extraction_exclude, wrapped_actions};
use crate::extraction::target_naming;
use crate::theme::desktop::{BUTTON_HEIGHT, MODAL_GROUP_GAP};
use crate::theme::palette;
use crate::theme::tokens::{FS_BODY, FS_SM, FS_XS, SPACE_3, SPACE_4, TOUCH_TARGET, Weight, font};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::frames::{self, Tone, px};
use crate::widgets::text::plain;
use crate::widgets::{field, number, toggle};
use crate::{
    Extraction, ExtractionScope, ExtractionSetup, ExtractionText as T, Model, TargetKind, Text,
    Texts, format,
};

const ACTIONS: [(ExtractionScope, T, Kind); 3] = [
    (ExtractionScope::Selected, T::ActionSelected, Kind::Outline),
    (ExtractionScope::Shown, T::ActionShown, Kind::Ghost),
    (ExtractionScope::All, T::ActionAll, Kind::Ghost),
];

const SCOPES: [(ExtractionScope, T); 3] = [
    (ExtractionScope::Selected, T::ScopeSelected),
    (ExtractionScope::Shown, T::ScopeShown),
    (ExtractionScope::All, T::ScopeAll),
];

pub(super) const POLICIES: [(ConflictPolicy, T); 3] = [
    (ConflictPolicy::Skip, T::PolicySkip),
    (ConflictPolicy::Overwrite, T::PolicyOverwrite),
    (ConflictPolicy::IfChanged, T::PolicyIfChanged),
];

const AUDIO: [(ExtractionAudio, T); 3] = [
    (ExtractionAudio::Ogg, T::AudioOgg),
    (ExtractionAudio::Wav, T::AudioWav),
    (ExtractionAudio::Raw, T::AudioRaw),
];

const MAX_MIP_LEVEL: u32 = 15;

enum Action {
    Submit,
    Close,
    ChooseFolder,
}

/// Returns whether the three Extraction actions fit in the line of `ui` beside `beside` points of
/// other controls, with the gaps between them. The widths of the labels are measured once per
/// language.
pub(super) fn fit_beside(ui: &Ui, model: &Model, beside: f32) -> bool {
    let id = ui
        .make_persistent_id("extraction-actions-width")
        .with(model.texts().language());
    let widths = ui.data(|data| data.get_temp::<f32>(id)).unwrap_or_else(|| {
        let texts = model.texts();
        let widths = ACTIONS
            .iter()
            .map(|(_, text, _)| button::width(ui, Size::Normal, &texts.get(*text)))
            .sum();
        ui.data_mut(|data| data.insert_temp(id, widths));
        widths
    });
    3.0f32.mul_add(ui.spacing().item_spacing.x, beside) + widths <= ui.available_width()
}

/// Shows the three Extraction actions in a line of their own, right-aligned; a line too narrow
/// for all three wraps the rest into the next one, which the room of the work surface is told.
pub(super) fn stacked_actions(ui: &mut Ui, model: &mut Model) {
    let size = vec2(ui.available_width(), ui.spacing().interact_size.y);
    let layout = Layout::right_to_left(Align::Center).with_main_wrap(true);
    let shown = ui.allocate_ui_with_layout(size, layout, |ui| actions(ui, model));
    wrapped_actions::record(ui.ctx(), shown.response.rect.height() > BUTTON_HEIGHT);
}

/// Shows the three Extraction actions in a right-to-left layout: no CTA in the main window, the
/// selected one as outline.
pub(super) fn actions(ui: &mut Ui, model: &mut Model) {
    let texts = model.texts();
    let running = model.extraction().is_some_and(Extraction::is_running);
    let mut begin = None;
    for (scope, text, kind) in ACTIONS.into_iter().rev() {
        let enabled = !running && model.scope_len(scope) > 0;
        if button::button(ui, kind, Size::Normal, &texts.get(text), enabled).clicked() {
            begin = Some(scope);
        }
    }
    if let Some(scope) = begin {
        model.begin_extraction(scope);
    }
}

pub(super) fn setup(ui: &Ui, model: &mut Model, requests: &mut Requests) {
    let Some(current) = model.extraction_setup() else {
        return;
    };
    let mut setup = current.clone();
    let title = model.texts().get(T::SetupTitle);
    let close_dialog = model.texts().get(Text::CloseDialog);
    let shown =
        frames::modal(frames::modal_id("extraction-setup"), Tone::Normal).show(ui.ctx(), |ui| {
            ui.set_width(frames::MODAL_WIDE_BODY_WIDTH);
            let closed = frames::head(ui, (&title, Tone::Normal), true, Some(&close_dialog));
            let action = if setup.is_confirming() {
                confirm(ui, model, &mut setup)
            } else {
                choices(ui, model, &mut setup)
            };
            if closed { Some(Action::Close) } else { action }
        });
    frames::modal_decoration(
        ui.ctx(),
        shown.response.layer_id,
        shown.response.rect,
        Tone::Normal,
    );
    let close = shown.should_close();
    if let Some(slot) = model.extraction_setup_mut() {
        *slot = setup;
    }
    match shown.inner {
        Some(Action::Submit) => {
            let ctx = ui.ctx().clone();
            requests.settings_changed |= model.submit_extraction(move || ctx.request_repaint());
        }
        Some(Action::Close) => model.close_extraction_setup(),
        Some(Action::ChooseFolder) => requests.dialog = Some(Dialog::ExtractionFolder),
        None if close => model.close_extraction_setup(),
        None => {}
    }
}

pub(super) fn group_label(ui: &mut Ui, text: &str) {
    ui.label(plain(text, FS_SM, Weight::Bold).color(palette::GRAY_1));
}

fn choices(ui: &mut Ui, model: &Model, setup: &mut ExtractionSetup) -> Option<Action> {
    let texts = model.texts();
    let (action, exclusion_accepted) = frames::scrolling_body(ui, "extraction-setup-body", |ui| {
        choice_groups(ui, model, setup)
    });
    let ready = setup.folder().is_some()
        && setup.refusal().is_none()
        && model.scope_len(setup.scope()) > 0
        && exclusion_accepted;
    let pressed = foot(ui, texts, (T::Start, ready), T::Close);
    action.or(match pressed {
        Some(true) => Some(Action::Submit),
        Some(false) => Some(Action::Close),
        None => None,
    })
}

/// Shows the scope, exclusion, target and Conversion choices; returns the action a control asked
/// for, and whether the exclusion text is accepted.
fn choice_groups(
    ui: &mut Ui,
    model: &Model,
    setup: &mut ExtractionSetup,
) -> (Option<Action>, bool) {
    let texts = model.texts();
    let mut action = None;
    group_label(ui, &texts.get(T::ScopeHeading));
    if let ExtractionScope::Changes(side) = setup.scope() {
        comparison_extraction::scope(ui, model, side);
    } else {
        for (scope, text) in SCOPES {
            let len = model.scope_len(scope);
            let label = texts.format(text, &[("number", number(texts, len))]);
            if toggle::radio(ui, setup.scope() == scope, &label, len > 0).clicked() {
                setup.set_scope(scope);
            }
        }
    }
    let exclusion_accepted = extraction_exclude::show(ui, texts, setup);
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(T::TargetHeading));
    for (kind, text) in [
        (TargetKind::NewFolder, T::TargetNew),
        (TargetKind::ExistingFolder, T::TargetExisting),
    ] {
        if toggle::radio(ui, setup.kind() == kind, &texts.get(text), true).clicked() {
            setup.set_kind(kind);
        }
    }
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            let choose = texts.get(T::ChooseFolder);
            if button::button(ui, Kind::Ghost, Size::Normal, &choose, true).clicked() {
                action = Some(Action::ChooseFolder);
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                folder_field(ui, texts, setup.folder());
            });
        });
    });
    refusal_notice(ui, texts, setup.refusal());
    writing(ui, model, setup);
    (action, exclusion_accepted)
}

/// Shows the name of a new folder or the Conflict policy for an existing one, then the
/// Conversion switch.
fn writing(ui: &mut Ui, model: &Model, setup: &mut ExtractionSetup) {
    let texts = model.texts();
    if setup.kind() == TargetKind::NewFolder {
        let name = new_folder_name(model, setup.scope());
        let hint = texts.format(T::TargetNewName, &[("name", name.into())]);
        ui.label(plain(hint, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT));
    } else {
        ui.add_space(MODAL_GROUP_GAP);
        group_label(ui, &texts.get(T::PolicyHeading));
        for (policy, text) in POLICIES {
            if toggle::radio(ui, setup.policy() == policy, &texts.get(text), true).clicked() {
                setup.set_policy(policy);
            }
        }
    }
    ui.add_space(MODAL_GROUP_GAP);
    let mut convert = setup.convert();
    if toggle::checkbox(ui, &mut convert, &texts.get(T::Conversion)).changed() {
        setup.set_convert(convert);
    }
    if convert {
        let mut textures = setup.textures();
        let mut audio = setup.audio();
        ui.indent("extraction-textures", |ui| {
            texture_choices(ui, texts, &mut textures);
            audio_choices(ui, texts, &mut audio);
        });
        setup.set_textures(textures);
        setup.set_audio(audio);
    }
}

/// Shows the sound formats of a Conversion and the note that soundbanks are unpacked.
fn audio_choices(ui: &mut Ui, texts: &Texts, audio: &mut ExtractionAudio) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = SPACE_4;
        let label = texts.get(T::AudioLabel);
        ui.label(plain(label, FS_SM, Weight::Regular).color(palette::GRAY_1));
        for (choice, text) in AUDIO {
            if toggle::radio(ui, *audio == choice, &texts.get(text), true).clicked() {
                *audio = choice;
            }
        }
    });
    let note = texts.get(T::AudioBanks);
    ui.label(plain(note, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT));
}

/// Shows the texture outputs of a Conversion: merge, PNG with its mip level, raw parts.
fn texture_choices(ui: &mut Ui, texts: &Texts, textures: &mut ExtractionTextures) {
    toggle::checkbox(ui, &mut textures.merge, &texts.get(T::TextureMerge));
    toggle::checkbox(ui, &mut textures.png, &texts.get(T::TexturePng));
    let png = textures.png;
    ui.indent("extraction-texture-mip", |ui| {
        let largest = &texts.get(T::TextureMipLargest);
        if toggle::radio(ui, textures.mip == 0, largest, png).clicked() {
            textures.mip = 0;
        }
        ui.horizontal(|ui| {
            let level_text = &texts.get(T::TextureMipLevel);
            if toggle::radio(ui, textures.mip > 0, level_text, png).clicked() && textures.mip == 0 {
                textures.mip = 1;
            }
            let mut level = textures.mip.max(1);
            number::number_field(ui, &mut level, 1..=MAX_MIP_LEVEL, png && textures.mip > 0);
            if textures.mip > 0 {
                textures.mip = level;
            }
        });
    });
    toggle::checkbox(ui, &mut textures.keep_raw, &texts.get(T::TextureRaw));
}

/// Shows the refusal with its established installation or unavailable identity boundary.
pub(super) fn refusal_notice(ui: &mut Ui, texts: &Texts, refusal: Option<&WriteProtectionError>) {
    let Some(refusal) = refusal else {
        return;
    };
    let notice = crate::installation::message(texts, refusal);
    frames::danger_alert(ui, |ui| {
        let text = plain(notice, FS_SM, Weight::Regular).color(palette::GRAY_1);
        ui.add(egui::Label::new(text).wrap());
    });
}

pub(super) fn folder_field(ui: &mut Ui, texts: &Texts, folder: Option<&Path>) {
    path_field(ui, texts.get(T::FolderNone), folder);
}

/// Shows the read-only field of a chosen path, with `none` as its hint while none is chosen.
pub(super) fn path_field(ui: &mut Ui, none: String, folder: Option<&Path>) {
    let shown = folder.map_or_else(String::new, |folder| folder.display().to_string());
    let mut text = shown.as_str();
    let bloom = field::bloom_slot(ui);
    let frame = field::frame_slot(ui);
    let edit = TextEdit::singleline(&mut text)
        .hint_text(plain(none, FS_BODY, Weight::Regular).italics())
        .font(font(FS_BODY, Weight::Regular))
        .frame(Frame::NONE.inner_margin(Margin::symmetric(px(SPACE_3), 0)))
        .min_size(vec2(0.0, TOUCH_TARGET))
        .desired_width(ui.available_width())
        .align(Align2::LEFT_CENTER);
    let response = ui.add(edit);
    field::paint_frame(ui, frame, response.rect, response.has_focus());
    if response.has_focus() {
        field::paint_bloom(ui, bloom, response.rect);
    }
}

fn confirm(ui: &mut Ui, model: &Model, setup: &mut ExtractionSetup) -> Option<Action> {
    let texts = model.texts();
    let asked = setup.confirmation()?;
    ui.label(texts.format(
        T::ConfirmAll,
        &[
            (
                "count",
                u64::try_from(asked.entries).unwrap_or(u64::MAX).into(),
            ),
            ("number", number(texts, asked.entries)),
            ("size", format::size(asked.bytes, texts.language()).into()),
        ],
    ));
    match foot(ui, texts, (T::ConfirmAllAction, true), T::Back) {
        Some(true) => Some(Action::Submit),
        Some(false) => {
            setup.withdraw_confirmation();
            None
        }
        None => None,
    }
}

/// Shows the foot of a modal: the ghost `secondary` and, right of it, the CTA `primary`. Returns
/// `Some(true)` for the CTA and `Some(false)` for the secondary action.
pub(super) fn foot(
    ui: &mut Ui,
    texts: &Texts,
    primary: (impl Into<Text>, bool),
    secondary: impl Into<Text>,
) -> Option<bool> {
    frames::foot_rule(ui);
    let mut pressed = None;
    let row = vec2(ui.available_width(), BUTTON_HEIGHT);
    ui.allocate_ui_with_layout(row, Layout::right_to_left(Align::Center), |ui| {
        let (cta, enabled) = primary;
        if button::button(ui, Kind::Cta, Size::Normal, &texts.get(cta), enabled).clicked() {
            pressed = Some(true);
        }
        let ghost = texts.get(secondary);
        if button::button(ui, Kind::Ghost, Size::Normal, &ghost, true).clicked() {
            pressed = Some(false);
        }
    });
    pressed
}

pub(super) fn new_folder_name(model: &Model, scope: ExtractionScope) -> String {
    let (channel, build) = match scope {
        ExtractionScope::Changes(side) => comparison_extraction::archive(model, side)
            .map(|archive| target_naming(archive.path(), archive.build())),
        _ => model.browser().map(|browser| {
            let index = browser.archive_index();
            target_naming(index.path(), index.build_of_archive())
        }),
    }
    .unwrap_or_default();
    default_target_name(channel.as_deref(), build, SystemTime::now())
}

pub(super) fn number(texts: &Texts, value: usize) -> FluentValue<'static> {
    format::number(u64::try_from(value).unwrap_or(u64::MAX), texts.language()).into()
}
