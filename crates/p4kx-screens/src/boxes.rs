// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::context::Context;
use crate::corners::{Corners, outline};
use crate::look::Look;
use crate::paint::Area;
use crate::widget::Widget;

const SAME: f32 = 1.0e-4;

/// Paints the background and the border of the box of a widget, with the rounded or chamfered
/// corners it has.
pub(crate) fn paint_box(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
) {
    let corners = Corners::of(&look.corners, area);
    if corners.is_square() {
        paint_square(cx, widget, look, area, alpha);
    } else {
        paint_rounded(cx, widget, look, area, alpha, &corners);
    }
}

fn paint_square(cx: &mut Context<'_>, widget: &Widget, look: &Look<'_>, area: Area, alpha: f32) {
    if let Some(colour) = look
        .background
        .and_then(|spec| cx.colour(spec, alpha, widget))
    {
        #[cfg(test)]
        let before = cx.painter.len();
        cx.painter.rect(area, colour);
        #[cfg(test)]
        observe_background(cx, before, colour);
    }
    paint_strips(cx, widget, look, area, alpha);
}

fn paint_strips(cx: &mut Context<'_>, widget: &Widget, look: &Look<'_>, area: Area, alpha: f32) {
    let [(top, _), (right, _), (bottom, _), (left, _)] = look.borders;
    let strips = [
        Area {
            height: top,
            ..area
        },
        Area {
            x: area.x + area.width - right,
            width: right,
            ..area
        },
        Area {
            y: area.y + area.height - bottom,
            height: bottom,
            ..area
        },
        Area {
            width: left,
            ..area
        },
    ];
    for (strip, (width, spec)) in strips.into_iter().zip(look.borders) {
        if width > 0.0
            && let Some(colour) = spec.and_then(|spec| cx.colour(spec, alpha, widget))
        {
            cx.painter.rect(strip, colour);
        }
    }
}

/// A border of one width and colour on all four sides is a ring between two contours; borders
/// that differ are drawn as the straight strips of a square box and listed.
fn paint_rounded(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
    corners: &Corners,
) {
    if let Some(colour) = look
        .background
        .and_then(|spec| cx.colour(spec, alpha, widget))
    {
        #[cfg(test)]
        let before = cx.painter.len();
        cx.painter.fill_outline(outline(area, corners), colour);
        #[cfg(test)]
        observe_background(cx, before, colour);
    }
    let widths = look.borders.map(|(width, _)| width);
    if widths.iter().all(|width| *width <= 0.0) {
        return;
    }
    let [first, ..] = look.borders;
    let uniform = look
        .borders
        .iter()
        .all(|(width, spec)| (width - first.0).abs() < SAME && *spec == first.1);
    if !uniform {
        cx.report.note(
            &widget.type_name,
            "borders that differ are drawn straight on a box with rounded corners",
            &widget.name,
        );
        paint_strips(cx, widget, look, area, alpha);
        return;
    }
    let [top, right, bottom, left] = widths;
    let mut ring = outline(area, corners);
    ring.extend(outline(
        area.inset(top, right, bottom, left),
        &corners.inside(widths),
    ));
    if let Some(colour) = first.1.and_then(|spec| cx.colour(spec, alpha, widget)) {
        cx.painter.fill_outline(ring, colour);
    }
}

#[cfg(test)]
const fn observe_background(cx: &mut Context<'_>, before: usize, colour: p4kx_gfx::Rgba) {
    if cx.painter.len() > before
        && let Some(trace) = &mut cx.power_trace
    {
        trace.background = Some(colour);
    }
}
