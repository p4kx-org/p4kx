// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{
    Color32, Context, ImageData, Label, Margin, Panel, Rect, Role, Sense, Stroke, StrokeKind,
    TextureHandle, TextureId, TextureOptions, Ui, WidgetInfo, pos2, vec2,
};
use fluent_bundle::FluentValue;

use super::source_origin;
use super::ui_tab::message;
use super::ui_tab_detail;
use crate::theme::desktop::{
    HAIRLINE, PREVIEW_HEIGHT, PREVIEW_MIN_HEIGHT, SPINNER_INLINE, TOOLBAR_PADDING,
};
use crate::theme::palette;
use crate::theme::tokens::{FS_SM, FS_XS, Weight};
use crate::theme::ui_metrics::UI_PICTURE_MAX_SCALE;
use crate::widgets::frames::{self, px};
use crate::widgets::indicator;
use crate::widgets::text::plain;
use crate::{Model, Texts, UiMode, UiRenderFailure, UiRendered, UiText};

/// Shows the picture of what is picked, and below it what it is made of and what it leaves out.
pub(super) fn show(ui: &mut Ui, model: &mut Model) {
    let tab = model.ui();
    let picked = match tab.mode() {
        UiMode::Files => tab.selected_file().is_some(),
        UiMode::Ships => tab.ship().and_then(crate::ShipPick::screen).is_some(),
    };
    if !picked {
        let texts = model.texts();
        let text = match tab.mode() {
            UiMode::Files => UiText::PickFile,
            UiMode::Ships if tab.ship().is_some() => UiText::PickScreen,
            UiMode::Ships => UiText::ScreensPick,
        };
        let text = texts.get(text);
        frames::panel().show(ui, |ui| message(ui, text));
        return;
    }
    Panel::bottom("ui-detail")
        .frame(frames::panel().inner_margin(Margin::same(px(TOOLBAR_PADDING))))
        .resizable(true)
        .default_size(PREVIEW_HEIGHT.min(ui.available_height() / 2.0))
        .min_size(PREVIEW_MIN_HEIGHT.min(ui.available_height() / 3.0))
        .show(ui, |ui| ui_tab_detail::show(ui, model));
    frames::panel().show(ui, |ui| {
        frames::preview_body().show(ui, |ui| picture(ui, model));
    });
}

fn picture(ui: &mut Ui, model: &Model) {
    let (texts, tab) = (model.texts(), model.ui());
    if let Some(path) = source_path(model) {
        ui.add(
            Label::new(plain(path, FS_XS, Weight::Regular).color(palette::GRAY_2_TEXT))
                .wrap()
                .selectable(true),
        );
    }
    if tab.is_rendering() {
        ui.horizontal(|ui| {
            indicator::spinner(ui, SPINNER_INLINE);
            ui.add(
                Label::new(
                    plain(texts.get(UiText::Rendering), FS_SM, Weight::Regular)
                        .color(palette::GRAY_1),
                )
                .selectable(false),
            );
        });
    }
    match tab.render_failure() {
        Some(UiRenderFailure::ArchiveChanged) => {
            frames::warning_alert(ui, |ui| {
                ui.add(Label::new(texts.get(UiText::RenderArchiveChanged)).selectable(true));
            });
        }
        Some(UiRenderFailure::Failed(Some(reason))) => {
            let text = texts.format(UiText::RenderFailed, &[("reason", reason.clone().into())]);
            frames::danger_alert(ui, |ui| {
                ui.add(Label::new(text).selectable(true));
            });
        }
        Some(UiRenderFailure::Failed(None)) => {
            frames::danger_alert(ui, |ui| {
                ui.add(Label::new(texts.get(UiText::RenderStopped)).selectable(true));
            });
        }
        None => {}
    }
    if let Some(rendered) = tab.rendered() {
        show_image(ui, texts, rendered);
    }
}

/// Returns the path of the picked file, or the record path of the picked screen.
fn source_path(model: &Model) -> Option<&str> {
    let tab = model.ui();
    match tab.mode() {
        UiMode::Files => tab.selected_file(),
        UiMode::Ships => tab.ship()?.screen().map(|screen| screen.canvas.as_str()),
    }
}

fn show_image(ui: &mut Ui, texts: &Texts, rendered: &UiRendered) {
    let (width, height) = (rendered.width(), rendered.height());
    let side = |value: usize| FluentValue::from(u64::try_from(value).unwrap_or(u64::MAX));
    let sizes = [("width", side(width)), ("height", side(height))];
    let limit = ui.ctx().input(|input| input.max_texture_side);
    if width == 0 || height == 0 || width > limit || height > limit {
        ui.add(Label::new(texts.format(UiText::TooLarge, &sizes)).selectable(true));
        return;
    }
    let texture = texture(ui.ctx(), rendered);
    let room = ui.available_size();
    let (area, response) = ui.allocate_exact_size(room, Sense::hover());
    let marker = texts.get(source_origin::label(Some(&rendered.origins().image)));
    response.widget_info(|| {
        let dimensions = texts.format(UiText::Shown, &sizes);
        WidgetInfo::labeled(Role::Image, true, format!("{dimensions}. {marker}"))
    });
    response.on_hover_text(marker);
    let native = vec2(points(width), points(height)) / ui.ctx().pixels_per_point();
    let scale = (area.width() / native.x)
        .min(area.height() / native.y)
        .min(UI_PICTURE_MAX_SCALE);
    let shown = Rect::from_center_size(area.center(), native * scale);
    let painter = ui.painter();
    painter.image(
        texture,
        shown,
        Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)),
        Color32::WHITE,
    );
    painter.rect_stroke(
        shown,
        0.0,
        Stroke::new(HAIRLINE, palette::GRAY_3),
        StrokeKind::Outside,
    );
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a picture side is at most 16384, far below the 2^24 an f32 holds exactly"
)]
const fn points(pixels: usize) -> f32 {
    pixels as f32
}

/// Returns the texture of `rendered`, uploaded once per picture.
fn texture(ctx: &Context, rendered: &UiRendered) -> TextureId {
    let id = egui::Id::unique("p4kx-ui-tab-picture");
    let cached = ctx.data(|data| data.get_temp::<(u64, TextureHandle)>(id));
    if let Some((serial, handle)) = cached
        && serial == rendered.serial()
    {
        return handle.id();
    }
    let handle = ctx.load_texture(
        "p4kx-ui-tab-picture",
        ImageData::Color(Arc::clone(rendered.image())),
        TextureOptions::LINEAR,
    );
    let texture = handle.id();
    ctx.data_mut(|data| data.insert_temp(id, (rendered.serial(), handle)));
    texture
}
