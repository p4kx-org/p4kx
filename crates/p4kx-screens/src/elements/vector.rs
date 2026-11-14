// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use p4kx_gfx::{Affine, ColorTransform, Paint, ShapeLayer};

use super::unsupported;
use crate::colour::{ColorSpec, scale_alpha};
use crate::context::Context;
use crate::images::{VectorEntry, vector_layers, vector_transform};
use crate::look::Look;
use crate::paint::{Area, TWIPS};
use crate::picture::{NineSlice, VectorFit, icon_preset_path};
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
    let path = look
        .style
        .picture
        .svg_path
        .as_deref()
        .unwrap_or(&widget.picture.vector.path);
    if let Some(preset) = widget
        .picture
        .icon_preset
        .as_deref()
        .filter(|_| path.is_empty())
    {
        draw(cx, widget, look, area, alpha, &icon_preset_path(preset));
        return;
    }
    if path.is_empty() {
        let reason = if draws_box(look) {
            "the box is drawn; the game sets the graphic at run time, the Record field svgPath is empty"
        } else {
            "names no vector graphic; the game sets it at run time, the Record field svgPath is empty"
        };
        cx.report.images_left_out += 1;
        cx.report.note(&widget.type_name, reason, &widget.name);
        return;
    }
    draw(cx, widget, look, area, alpha, path);
}

/// Returns whether the style gives the shape a background or a border: a shape without a graphic
/// is then the box that draws.
fn draws_box(look: &Look<'_>) -> bool {
    look.background.is_some() || look.borders.iter().any(|(width, _)| *width > 0.0)
}

fn overlay_colours(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    alpha: f32,
) -> Option<ColorTransform> {
    let spec = look
        .fill
        .or(widget.picture.vector.colour.as_ref())
        .or(widget.picture.colour.as_ref())?;
    let colour = cx.colour(spec, alpha, widget)?;
    Some(ColorTransform {
        multiply: [0.0, 0.0, 0.0, f32::from(colour.alpha) / 255.0],
        add: [
            f32::from(colour.red),
            f32::from(colour.green),
            f32::from(colour.blue),
            0.0,
        ],
    })
}

fn slotted(cx: &Context<'_>, entry: &VectorEntry) -> Vec<ShapeLayer> {
    entry
        .vector
        .layers
        .iter()
        .zip(&entry.vector.slots)
        .map(|(layer, slot)| {
            let mut layer = layer.clone();
            if let (Some(slot), Paint::Solid(own)) = (slot, &layer.paint) {
                let spec = ColorSpec::Slot {
                    name: slot.name.clone(),
                    alpha: slot.opacity,
                };
                if let Ok(colour) = cx.palette.resolve(&spec, 1.0) {
                    let own_alpha = f32::from(own.alpha) / 255.0;
                    layer.paint = Paint::Solid(scale_alpha(colour, own_alpha));
                }
            }
            layer
        })
        .collect()
}

fn shape_of(cx: &mut Context<'_>, entry: &VectorEntry) -> usize {
    entry.shape.get().unwrap_or_else(|| {
        let shape = cx.painter.add_shape(entry.vector.layers.clone());
        entry.shape.set(Some(shape));
        shape
    })
}

/// Draws the vector graphic at `record_path` into `area` the way the widget fits it.
pub(super) fn draw(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
    record_path: &str,
) {
    let entry = match cx.images.vector(record_path) {
        Ok(entry) => entry,
        Err(why) => {
            cx.report.images_left_out += 1;
            cx.report.note(
                &widget.type_name,
                "vector graphic missing or unreadable",
                &format!("{record_path}: {why}"),
            );
            return;
        }
    };
    if !entry.reported.replace(true) {
        for note in &entry.vector.notes {
            cx.report.note(&widget.type_name, note, record_path);
        }
    }
    let item = cx.painter.len();
    if place(cx, widget, look, area, alpha, &entry) {
        cx.report.images_drawn += 1;
        if cx.painter.len() > item
            && let Some(inputs) = cx.inputs.as_mut()
        {
            inputs.vectors.insert(item, record_path.to_owned());
        }
    }
}

struct Placing<'a> {
    area: Area,
    fit: VectorFit,
    place: [f32; 2],
    flip: [bool; 2],
    slice: Option<&'a NineSlice>,
    by_slot: bool,
}

fn place(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
    entry: &VectorEntry,
) -> bool {
    let spec = &widget.picture.vector;
    if !spec.visible {
        return false;
    }
    let style = &look.style.picture;
    let overlay = style.overlay.unwrap_or(spec.overlay);
    let tinted = overlay
        .then(|| overlay_colours(cx, widget, look, alpha))
        .flatten();
    let colours = tinted.unwrap_or_else(|| ColorTransform {
        multiply: [1.0, 1.0, 1.0, alpha.clamp(0.0, 1.0)],
        add: [0.0; 4],
    });
    if colours.multiply[3] <= 0.0 {
        return false;
    }
    let by_slot = tinted.is_none() && entry.vector.slots.iter().any(Option::is_some);
    let stretching = matches!(
        spec.fit,
        VectorFit::StretchXY | VectorFit::StretchX | VectorFit::StretchY
    );
    let sliced = style.nine_slice.unwrap_or(spec.nine_slice_enabled) && stretching;
    if (by_slot || sliced) && !cx.images.charge_copy(entry.vector.commands) {
        cx.report.images_left_out += 1;
        cx.report.note(
            &widget.type_name,
            "the copies of vector graphics for slots and nine-slices exceed the path budget",
            &widget.name,
        );
        return false;
    }
    let placing = Placing {
        area,
        fit: spec.fit,
        place: spec.place,
        flip: [
            style.svg_flip[0].unwrap_or(spec.flip[0]),
            style.svg_flip[1].unwrap_or(spec.flip[1]),
        ],
        slice: spec.nine_slice.as_ref().filter(|_| sliced),
        by_slot,
    };
    put(cx, entry, &placing, colours);
    true
}

fn put(cx: &mut Context<'_>, entry: &VectorEntry, how: &Placing<'_>, colours: ColorTransform) {
    let view = entry.vector.view;
    let layers: Cow<'_, [ShapeLayer]> = if how.by_slot {
        Cow::Owned(slotted(cx, entry))
    } else {
        Cow::Borrowed(&entry.vector.layers)
    };
    let sliced = how.slice.and_then(|slice| {
        vector_layers(
            &layers,
            view,
            slice,
            how.area,
            (how.fit, how.place),
            how.flip,
        )
    });
    if let Some(sliced) = sliced {
        let shape = cx.painter.add_shape(sliced.layers);
        let to_stage = Affine {
            a: TWIPS,
            b: 0.0,
            c: 0.0,
            d: TWIPS,
            tx: sliced.origin[0] * TWIPS,
            ty: sliced.origin[1] * TWIPS,
        };
        cx.painter.place(shape, to_stage, colours);
        return;
    }
    let shape = if how.by_slot {
        cx.painter.add_shape(layers.into_owned())
    } else {
        shape_of(cx, entry)
    };
    let transform = vector_transform(how.area, view, how.fit, how.place, how.flip);
    cx.painter.place(shape, transform, colours);
}
