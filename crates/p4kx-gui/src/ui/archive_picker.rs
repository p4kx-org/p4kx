// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use egui::{Popup, Rect, Role, Sense, Stroke, StrokeKind, Ui, WidgetInfo, pos2, vec2};
use p4kx_channels::Channel;

use super::menu::{dropdown, entry};
use super::search::eyebrow;
use super::status::build_label;
use crate::theme::desktop::{
    BUTTON_HEIGHT, CHEVRON, PICKER_CHEVRON_INSET, PICKER_MIN_WIDTH, PICKER_PADDING_RIGHT,
    PICKER_PADDING_X,
};
use crate::theme::palette;
use crate::theme::tokens::{CORNER_CONTROL, FS_BODY, GLOW_PRIMARY, Weight};
use crate::widgets::rows::chevron;
use crate::widgets::text::{line, paint_left, plain};
use crate::{ComparisonText, Model, Text, Texts};

/// What the user did with a picker.
pub(super) enum Picked {
    /// Picked this Archive from the list.
    Archive(PathBuf),
    /// Asked for a file dialog.
    ChooseFile,
}

/// Shows the labelled picker of one side, `width` wide, showing `current`; offers the detected
/// Channels, the Recent Archives and a file dialog.
pub(super) fn show(
    ui: &mut Ui,
    model: &Model,
    label: &str,
    current: Option<&Path>,
    width: f32,
) -> Option<Picked> {
    let texts = model.texts();
    let channels = model.channels().detected();
    let width = width.max(PICKER_MIN_WIDTH);
    let mut picked = None;
    ui.vertical(|ui| {
        ui.set_width(width);
        let caption = ui.label(eyebrow(label));
        let (rect, response) = ui.allocate_exact_size(vec2(width, BUTTON_HEIGHT), Sense::click());
        let response = response.labelled_by(caption.id);
        let display = describe(texts, channels, current);
        response.widget_info(|| {
            let mut info = WidgetInfo::labeled(Role::ComboBox, true, label);
            info.current_text_value = Some(display.text.clone());
            info
        });
        let open = Popup::is_id_open(ui.ctx(), Popup::default_response_id(&response));
        paint_field(ui, rect, &display, response.has_focus() || open);
        Popup::menu(&response).width(width).show(|ui| {
            dropdown(ui);
            ui.set_min_width(width);
            picked = list(ui, texts, model, channels);
        });
    });
    picked
}

/// What a picker shows: its text, and whether that is the hint before a pick.
struct Display {
    text: String,
    hint: bool,
}

fn describe(texts: &Texts, channels: &[Channel], current: Option<&Path>) -> Display {
    let Some(path) = current else {
        return Display {
            text: texts.get(ComparisonText::PickArchive),
            hint: true,
        };
    };
    let text = channels
        .iter()
        .find(|channel| channel.archive() == path)
        .map_or_else(
            || path.display().to_string(),
            |channel| side(texts, channel),
        );
    Display { text, hint: false }
}

fn side(texts: &Texts, channel: &Channel) -> String {
    texts.format(
        ComparisonText::Side,
        &[
            ("channel", channel.name().to_owned().into()),
            ("build", build_label(texts, channel.build()).into()),
        ],
    )
}

/// Paints the field after the theme's `select`: control fill and border, orange border
/// and glow while focused or open, the value in 16 point Regular, the hint in italics.
fn paint_field(ui: &Ui, rect: Rect, display: &Display, focused: bool) {
    let border = if focused {
        ui.painter()
            .add(GLOW_PRIMARY.as_shape(rect, CORNER_CONTROL));
        palette::PRIMARY
    } else {
        palette::CONTROL_BORDER
    };
    ui.painter().rect(
        rect,
        CORNER_CONTROL,
        palette::CONTROL_BG,
        Stroke::new(1.0, border),
        StrokeKind::Inside,
    );
    let centre = pos2(
        rect.right() - PICKER_CHEVRON_INSET - CHEVRON / 2.0,
        rect.center().y,
    );
    chevron(ui.painter(), centre, CHEVRON, true, palette::PRIMARY);
    let area = Rect::from_min_max(
        pos2(rect.left() + PICKER_PADDING_X, rect.top()),
        pos2(rect.right() - PICKER_PADDING_RIGHT, rect.bottom()),
    );
    let mut text = plain(display.text.as_str(), FS_BODY, Weight::Regular);
    let colour = if display.hint {
        text = text.italics();
        palette::GRAY_2_TEXT
    } else {
        palette::GRAY_1
    };
    let galley = line(ui, text, area.width());
    paint_left(ui, area, galley, colour);
}
fn list(ui: &mut Ui, texts: &Texts, model: &Model, channels: &[Channel]) -> Option<Picked> {
    let mut picked = None;
    let heading = |ui: &mut Ui, text: Text| {
        ui.horizontal(|ui| {
            ui.add_space(PICKER_PADDING_X);
            ui.label(eyebrow(&texts.get(text)));
        });
    };
    heading(ui, Text::ChannelsHeading);
    if channels.is_empty() {
        note(ui, &texts.get(Text::ChannelsNone));
    }
    for channel in channels {
        if ui.button(entry(&side(texts, channel))).clicked() {
            picked = Some(Picked::Archive(channel.archive().to_path_buf()));
        }
    }
    ui.separator();
    heading(ui, Text::MenuRecent);
    if model.recent().is_empty() {
        note(ui, &texts.get(Text::RecentNone));
    }
    for path in model.recent() {
        if ui.button(entry(&path.display().to_string())).clicked() {
            picked = Some(Picked::Archive(path.clone()));
        }
    }
    ui.separator();
    if ui
        .button(entry(&texts.get(ComparisonText::ChooseFile)))
        .clicked()
    {
        picked = Some(Picked::ChooseFile);
    }
    picked
}

fn note(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        ui.add_space(PICKER_PADDING_X - ui.spacing().item_spacing.x);
        ui.label(entry(text).italics().color(palette::GRAY_2_TEXT));
    });
}
