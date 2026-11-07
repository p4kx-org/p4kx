// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use egui::{Align, Align2, Frame, Key, Layout, Margin, TextEdit, Ui, vec2};

use super::extraction::{foot, group_label, path_field};
use super::regions;
use crate::dialog::Dialog;
use crate::output_target::{NameProblem, PickerFailure, Selection, TargetSelection};
use crate::theme::desktop::MODAL_GROUP_GAP;
use crate::theme::palette;
use crate::theme::tokens::{FS_BODY, FS_SM, SPACE_3, TOUCH_TARGET, Weight, font};
use crate::widgets::button::{self, Kind, Size};
use crate::widgets::field;
use crate::widgets::frames::{self, Tone, px};
use crate::widgets::placement;
use crate::widgets::text::plain;
use crate::{OutputTargetText as T, Text, Texts};

#[derive(Debug, Clone, Copy, Default)]
struct Actions {
    parent: bool,
    confirm: bool,
    cancel: bool,
}

pub(crate) fn show(ui: &Ui, texts: &Texts, state: &mut TargetSelection) -> Option<Dialog> {
    let pending = state.pending_mut()?;
    let purpose = pending.dialog();
    let title = texts.get(T::Title);
    let close = texts.get(Text::CloseDialog);
    let shown = placement::hanging_modal(ui.ctx(), frames::modal_id("output-target"), Tone::Normal)
        .show(ui.ctx(), |ui| {
            ui.set_width(frames::MODAL_WIDE_BODY_WIDTH);
            let closed = frames::head(ui, (&title, Tone::Normal), true, Some(&close));
            let mut actions = body(ui, texts, pending);
            actions.cancel |= closed;
            actions
        });
    frames::modal_decoration(
        ui.ctx(),
        shown.response.layer_id,
        shown.response.rect,
        Tone::Normal,
    );
    let actions = shown.inner;
    if actions.cancel || shown.should_close() {
        state.cancel();
    } else if actions.confirm {
        if state.confirm() {
            ui.ctx().request_repaint();
        }
    } else if actions.parent {
        return Some(purpose);
    }
    None
}

#[derive(Debug, Clone, Copy)]
struct Body {
    parent: bool,
    enter: bool,
    ready: bool,
}

fn body(ui: &mut Ui, texts: &Texts, pending: &mut Selection) -> Actions {
    let room = placement::body_room(ui);
    let shown = frames::scrolling_body_in(ui, ("output-target-body", room), |ui| {
        fields(ui, texts, pending)
    });
    regions::record(ui.ctx(), |regions| {
        regions.scrolls = shown.content_size.y > shown.inner_rect.height();
    });
    let body = shown.inner;
    let confirm = if pending.dialog().is_setup_output() {
        T::Confirm
    } else {
        T::StartExport
    };
    let pressed = foot(ui, texts, (confirm, body.ready), T::Cancel);
    Actions {
        parent: body.parent,
        confirm: body.ready && (body.enter || pressed == Some(true)),
        cancel: pressed == Some(false),
    }
}

fn fields(ui: &mut Ui, texts: &Texts, pending: &mut Selection) -> Body {
    let mut parent = false;
    ui.label(plain(texts.get(T::ParentHint), FS_SM, Weight::Regular));
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(T::Parent));
    ui.horizontal(|ui| {
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            parent = button::button(
                ui,
                Kind::Ghost,
                Size::Normal,
                &texts.get(T::ChooseParent),
                true,
            )
            .clicked();
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                path_field(ui, texts.get(T::NoParent), pending.parent());
            });
        });
    });
    ui.add_space(MODAL_GROUP_GAP);
    let enter = name(ui, texts, pending);
    notices(ui, texts, pending);
    ui.add_space(MODAL_GROUP_GAP);
    group_label(ui, &texts.get(T::Destination));
    let path = pending.path();
    ui.add(
        egui::Label::new(plain(
            path.as_deref().map_or_else(
                || texts.get(T::NoDestination),
                |path| path.display().to_string(),
            ),
            FS_SM,
            Weight::Regular,
        ))
        .wrap(),
    );
    Body {
        parent,
        enter,
        ready: path.is_some(),
    }
}

fn name(ui: &mut Ui, texts: &Texts, pending: &mut Selection) -> bool {
    let label = ui.label(super::search::eyebrow(&texts.get(
        if pending.dialog().is_package_output() {
            T::PackageName
        } else {
            T::FileName
        },
    )));
    let first = pending.take_focus();
    let bloom = field::bloom_slot(ui);
    let frame = field::frame_slot(ui);
    let edit = TextEdit::singleline(pending.name_mut())
        .font(font(FS_BODY, Weight::Regular))
        .frame(Frame::NONE.inner_margin(Margin::symmetric(px(SPACE_3), 0)))
        .min_size(vec2(0.0, TOUCH_TARGET))
        .desired_width(ui.available_width())
        .align(Align2::LEFT_CENTER);
    let response = ui.add(edit).labelled_by(label.id);
    field::paint_frame(ui, frame, response.rect, response.has_focus());
    if response.has_focus() {
        field::paint_bloom(ui, bloom, response.rect);
    }
    if first {
        response.request_focus();
    }
    response.lost_focus() && ui.input(|input| input.key_pressed(Key::Enter))
}

fn notices(ui: &mut Ui, texts: &Texts, pending: &Selection) {
    let hint = if pending.dialog().is_package_output() {
        T::PackageHint
    } else {
        T::FileHint
    };
    ui.add(
        egui::Label::new(
            plain(texts.get(hint), FS_SM, Weight::Regular).color(palette::GRAY_2_TEXT),
        )
        .wrap(),
    );
    if pending.dialog() == Dialog::DiagnosticCatalogFile {
        frames::warning_alert(ui, |ui| {
            ui.add(
                egui::Label::new(plain(texts.get(T::DiagnosticHint), FS_SM, Weight::Regular))
                    .wrap(),
            );
        });
    }
    let problem = pending.name_problem().map(|problem| {
        texts.get(match problem {
            NameProblem::Empty => T::Empty,
            NameProblem::Component => T::Component,
            NameProblem::Control => T::Control,
        })
    });
    let picker = pending.picker_failure().map(|failure| match failure {
        PickerFailure::Spawn(error) => {
            texts.format(T::PickerFailed, &[("reason", error.to_string().into())])
        }
        PickerFailure::Disconnected => texts.get(T::PickerDisconnected),
        PickerFailure::RelativePath => texts.get(T::PickerRelative),
    });
    for message in [problem, picker].into_iter().flatten() {
        frames::danger_alert(ui, |ui| {
            ui.add(egui::Label::new(plain(message, FS_SM, Weight::Regular)).wrap());
        });
    }
}

#[cfg(test)]
mod interaction_tests;
#[cfg(test)]
mod test_fixture;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod small_window_tests;
