// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_gfx::ColorTransform;

use super::{unsupported, vector};
use crate::context::Context;
use crate::images::{archive_texture_path, texture_rect};
use crate::look::Look;
use crate::paint::Area;
use crate::widget::Widget;

pub(super) fn paint(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
) {
    if !cx.images.is_enabled() {
        cx.report.images_left_out += 1;
        unsupported::note(cx, widget);
        return;
    }
    if area.is_empty() {
        return;
    }
    let style = &look.style.picture;
    let texture = style
        .image_path
        .as_deref()
        .unwrap_or(&widget.picture.texture);
    if !texture.is_empty() {
        draw(cx, widget, look, area, alpha, texture);
        return;
    }
    let svg = style
        .svg_path
        .as_deref()
        .unwrap_or(&widget.picture.vector.path);
    if svg.is_empty() {
        return;
    }
    vector::draw(cx, widget, look, area, alpha, svg);
}

fn tint(cx: &mut Context<'_>, widget: &Widget, look: &Look<'_>, alpha: f32) -> ColorTransform {
    let spec = look.fill.or(widget.picture.colour.as_ref());
    let plain = [1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0)];
    let [red, green, blue, factor] =
        spec.and_then(|spec| cx.colour(spec, alpha, widget))
            .map_or(plain, |colour| {
                [
                    f32::from(colour.red) / 255.0,
                    f32::from(colour.green) / 255.0,
                    f32::from(colour.blue) / 255.0,
                    f32::from(colour.alpha) / 255.0,
                ]
            });
    ColorTransform {
        multiply: [red, green, blue, factor],
        add: [0.0; 4],
    }
}

/// Draws the texture of the Record path `record_path` into `area` the way the widget scales it.
pub(super) fn draw(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
    record_path: &str,
) {
    let path = archive_texture_path(record_path);
    let texture = match cx.images.texture(&path) {
        Ok(texture) => texture,
        Err(why) => {
            cx.report.images_left_out += 1;
            cx.report.missing_texture(&path);
            cx.report.note(
                &widget.type_name,
                "texture missing or unreadable",
                &format!("{path}: {why}"),
            );
            return;
        }
    };
    let style = &look.style.picture;
    let flip = [
        style.image_flip[0].unwrap_or(widget.picture.flip[0]),
        style.image_flip[1].unwrap_or(widget.picture.flip[1]),
    ];
    let rect = texture_rect(
        area,
        (texture.width, texture.height),
        widget.picture.fit,
        widget.picture.place,
    );
    let colours = tint(cx, widget, look, alpha);
    if colours.multiply[3] <= 0.0 {
        return;
    }
    if let Some(inputs) = cx.inputs.as_mut() {
        inputs
            .textures
            .entry(texture.id)
            .or_insert_with(|| path.into_owned());
    }
    cx.painter.bitmap(
        texture.id,
        (texture.width, texture.height),
        rect.image,
        rect.clip,
        flip,
        colours,
    );
    cx.report.images_drawn += 1;
}
