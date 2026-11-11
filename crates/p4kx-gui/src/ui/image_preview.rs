// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use egui::{
    Color32, Id, Rect, Role, ScrollArea, Sense, TextureId, Ui, Vec2, WidgetInfo, pos2, vec2,
};
use fluent_bundle::FluentValue;
use p4kx_extraction::FieldOrigin;

use super::search::eyebrow;
use crate::theme::desktop::{IMAGE_LABEL_GAP, IMAGE_SELECTOR_GAP, ROW_HEIGHT, SPINNER_INLINE};
use crate::theme::palette;
use crate::widgets::frames;
use crate::widgets::indicator::spinner;
use crate::widgets::scroll_region;
use crate::widgets::summary::{Fact, FactsStrip};
use crate::widgets::{tab, tooltip};
use crate::{
    ImageHeaderProvenance, ImageMipProvenance, ImagePreview, ImageStatus, ImageText, Language,
    MipStatus, ScrollText, Text, Texts, TextureInfo,
};

/// Shows the mip selector of an opened texture: one segment per mip a texture may be on this
/// display, labelled with the mip's longer side, `level` pressed. Returns the level clicked.
fn mip_selector(
    ui: &mut Ui,
    texts: &Texts,
    info: &TextureInfo,
    level: Option<u32>,
    origins: Option<&ImageHeaderProvenance>,
) -> Option<u32> {
    let first = info.first_showable_level(max_side(ui))?;
    let labels = info.mip_labels().get(first as usize..)?;
    let active = level
        .and_then(|level| level.checked_sub(first))
        .map(|offset| offset as usize);
    let mut clicked = None;
    let selector = ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = IMAGE_LABEL_GAP;
        ui.label(eyebrow(&texts.get(ImageText::Mip)));
        clicked = tab::segments(ui, labels, active);
    });
    if let Some(origins) = origins {
        tooltip::hover(selector.response, |ui| {
            tooltip::line(ui, origin_text(texts, &origins.mip_label));
        });
    }
    ui.add_space(IMAGE_SELECTOR_GAP);
    clicked
        .and_then(|index| u32::try_from(index).ok())
        .map(|offset| first + offset)
}

/// Shows the Image tab of the texture at `path`: its facts, the mip selector and the shown mip.
/// Returns the level clicked, or the one that fits the view when no mip was chosen yet.
pub(super) fn show(ui: &mut Ui, texts: &Texts, path: &str, image: &ImagePreview) -> Option<u32> {
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    let status = image.status();
    if let ImageStatus::Opened(info, _) = status {
        let rect = facts(ui, texts, image.serial(), info).show(ui);
        if let Some(origins) = image.header_provenance() {
            let response = ui.interact(rect, ui.scope_id().with("texture-origins"), Sense::hover());
            tooltip::hover(response, |ui| header_origins(ui, texts, info, origins));
        }
    }
    frames::preview_body()
        .show(ui, |ui| match status {
            ImageStatus::Opening => {
                loading(ui, &texts.format(Text::PreviewLoading, &[path_arg()]));
                None
            }
            ImageStatus::Failed(reason) => {
                ui.colored_label(palette::DANGER_TEXT, failure(texts, path, reason));
                None
            }
            ImageStatus::ArchiveChanged => {
                ui.colored_label(palette::WARNING, texts.get(Text::PreviewArchiveChanged));
                None
            }
            ImageStatus::Opened(info, _) if !info.is_decodable() => {
                let format = ("format", FluentValue::from(info.format().to_owned()));
                let text = texts.format(ImageText::Unsupported, &[format, path_arg()]);
                ui.colored_label(palette::DANGER_TEXT, text);
                None
            }
            ImageStatus::Opened(info, mip) => {
                let clicked =
                    mip_selector(ui, texts, info, image.level(), image.header_provenance());
                let fitted = mip_body(ui, texts, path, info, mip, image.mip_provenance());
                clicked.or(fitted)
            }
        })
        .inner
}

fn mip_body(
    ui: &mut Ui,
    texts: &Texts,
    path: &str,
    info: &TextureInfo,
    mip: MipStatus<'_>,
    origins: Option<&ImageMipProvenance>,
) -> Option<u32> {
    match mip {
        MipStatus::Unchosen if info.first_showable_level(max_side(ui)).is_none() => {
            let path = ("path", FluentValue::from(path.to_owned()));
            let text = texts.format(ImageText::Unshowable, &[path]);
            ui.colored_label(palette::DANGER_TEXT, text);
            None
        }
        MipStatus::Unchosen => {
            let view = settled_view(ui)?;
            info.fitting_level([pixels(view.x), pixels(view.y)], max_side(ui))
        }
        MipStatus::Decoding(level) | MipStatus::Decoded(level, None) => {
            let args = [
                ("level", FluentValue::from(level.to_string())),
                ("path", FluentValue::from(path.to_owned())),
            ];
            loading(ui, &texts.format(ImageText::Decoding, &args));
            None
        }
        MipStatus::Failed(_, reason) => {
            ui.colored_label(palette::DANGER_TEXT, failure(texts, path, reason));
            None
        }
        MipStatus::Decoded(level, Some(texture)) => {
            if let Some(size) = info.image_size(level) {
                picture(ui, texts, (level, size), texture, origins);
            }
            None
        }
    }
}

fn picture(
    ui: &mut Ui,
    texts: &Texts,
    (level, [width, height]): (u32, [u32; 2]),
    texture: TextureId,
    origins: Option<&ImageMipProvenance>,
) {
    let size = vec2(points(width), points(height)) / ui.ctx().pixels_per_point();
    let scroll = ScrollArea::both()
        .id_salt("preview-image")
        .auto_shrink(false);
    let id = ui.scope_id().with("preview-image");
    scroll_region::show(
        ui,
        (id, || texts.get(ScrollText::Image)),
        ROW_HEIGHT,
        scroll,
        |scroll, ui| {
            scroll.show(ui, |ui| {
                let area = size.max(ui.available_size());
                let (rect, response) = ui.allocate_exact_size(area, Sense::hover());
                response.widget_info(|| {
                    let args = [
                        ("level", FluentValue::from(level.to_string())),
                        ("width", FluentValue::from(width.to_string())),
                        ("height", FluentValue::from(height.to_string())),
                    ];
                    WidgetInfo::labeled(Role::Image, true, texts.format(ImageText::Shown, &args))
                });
                let shown = Rect::from_center_size(rect.center(), size);
                let whole = Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0));
                ui.painter().image(texture, shown, whole, Color32::WHITE);
                if let Some(origins) = origins {
                    tooltip::hover(response, |ui| mip_origins(ui, texts, origins));
                }
            })
        },
    );
}

fn origin_text(texts: &Texts, origin: &FieldOrigin) -> String {
    texts.get(match origin.contains_encrypted_source() {
        Some(true) => ImageText::OriginEncrypted,
        Some(false) => ImageText::OriginPlain,
        None => ImageText::OriginUnknown,
    })
}

fn origin_row(ui: &mut Ui, texts: &Texts, field: ImageText, origin: &FieldOrigin) {
    ui.horizontal(|ui| {
        ui.strong(texts.get(field));
        tooltip::line(ui, origin_text(texts, origin));
    });
}

fn header_origins(ui: &mut Ui, texts: &Texts, info: &TextureInfo, origins: &ImageHeaderProvenance) {
    for (field, origin) in [
        (ImageText::Format, &origins.format),
        (ImageText::Size, &origins.width),
        (ImageText::Mips, &origins.mip_count),
        (ImageText::Faces, &origins.cubemap_faces),
    ] {
        origin_row(ui, texts, field, origin);
    }
    if info.layers().is_some() {
        origin_row(ui, texts, ImageText::Layers, &origins.layers);
    }
}

fn mip_origins(ui: &mut Ui, texts: &Texts, origins: &ImageMipProvenance) {
    for (field, origin) in [
        (ImageText::Mip, &origins.level),
        (ImageText::Size, &origins.width),
        (ImageText::Pixels, &origins.pixels),
    ] {
        origin_row(ui, texts, field, origin);
    }
}

/// Claims the space left for the image and returns its size in pixels once it is the same in
/// two frames in a row.
fn settled_view(ui: &mut Ui) -> Option<Vec2> {
    let view = ui.available_size() * ui.ctx().pixels_per_point();
    let id = ui.scope_id().with("p4kx-image-view");
    let previous = ui.data_mut(|data| {
        let previous = data.get_temp::<Vec2>(id);
        data.insert_temp(id, view);
        previous
    });
    ScrollArea::both()
        .id_salt("preview-image")
        .auto_shrink(false)
        .show(ui, |_| {});
    if previous != Some(view) {
        ui.ctx().request_repaint();
    }
    previous.filter(|&previous| previous == view)
}

fn loading(ui: &mut Ui, text: &str) {
    ui.horizontal(|ui| {
        spinner(ui, SPINNER_INLINE);
        ui.label(text);
    });
}

fn failure(texts: &Texts, path: &str, reason: Option<&str>) -> String {
    let path_arg = || ("path", FluentValue::from(path.to_owned()));
    reason.map_or_else(
        || texts.format(Text::PreviewStopped, &[path_arg()]),
        |reason| {
            let reason = ("reason", FluentValue::from(reason.to_owned()));
            texts.format(ImageText::Failed, &[path_arg(), reason])
        },
    )
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct Key {
    serial: u64,
    language: Language,
    pixels_per_point: f32,
}

/// Returns the facts of `info`, laid out once per image preview, language and scale.
fn facts(ui: &Ui, texts: &Texts, serial: u64, info: &TextureInfo) -> Arc<FactsStrip> {
    let id = Id::unique("p4kx-image-facts");
    let key = Key {
        serial,
        language: texts.language(),
        pixels_per_point: ui.ctx().pixels_per_point(),
    };
    if let Some((cached, strip)) = ui.data(|data| data.get_temp::<(Key, Arc<FactsStrip>)>(id))
        && cached == key
    {
        return strip;
    }
    let dimensions = texts.format(
        ImageText::Dimensions,
        &[
            ("width", info.width().to_string().into()),
            ("height", info.height().to_string().into()),
        ],
    );
    let mips = info.mip_count().to_string();
    let faces = info
        .cubemap_faces()
        .map_or_else(|| texts.get(ImageText::None), |faces| faces.to_string());
    let layers = info.layers().map(|layers| layers.to_string());
    let keys = [
        ImageText::Format,
        ImageText::Size,
        ImageText::Mips,
        ImageText::Faces,
        ImageText::Layers,
    ]
    .map(|text| texts.get(text));
    let values = [
        Some(info.format()),
        Some(dimensions.as_str()),
        Some(mips.as_str()),
        Some(faces.as_str()),
        layers.as_deref(),
    ];
    let laid: Vec<Fact<'_>> = keys
        .iter()
        .zip(values)
        .filter_map(|(key, value)| Some(Fact { key, value: value? }))
        .collect();
    let strip = Arc::new(FactsStrip::lay_out(ui, &laid));
    ui.data_mut(|data| data.insert_temp(id, (key, Arc::clone(&strip))));
    strip
}

/// Returns the longest side, in pixels, of a texture the window can show.
pub(super) fn max_side(ui: &Ui) -> u32 {
    let side = ui.ctx().input(|input| input.max_texture_side);
    u32::try_from(side).unwrap_or(u32::MAX)
}

#[expect(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    reason = "the view is a few thousand pixels at most, and negative sizes count as none"
)]
const fn pixels(length: f32) -> u32 {
    length.max(0.0) as u32
}

#[expect(
    clippy::cast_precision_loss,
    reason = "mip sides stay far below the 2^24 an f32 holds exactly"
)]
const fn points(pixels: u32) -> f32 {
    pixels as f32
}
