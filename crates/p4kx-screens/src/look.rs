// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::rc::Rc;

use crate::colour::ColorSpec;
use crate::corners::Corner;
use crate::style::Modifiers;
use crate::text::{FontStyleInfo, Shaped};
use crate::widget::Widget;

fn corners_of(widget: &Widget, style: &Modifiers) -> [Corner; 4] {
    let over = &style.geometry;
    let mut corners = widget.corners;
    for (at, corner) in corners.iter_mut().enumerate() {
        if let Some(radius) = over.radius.get(at).copied().flatten() {
            corner.radius = corner.radius.with_amount(radius);
        }
        if let Some(chamfer) = over.chamfer.get(at).copied().flatten() {
            corner.chamfer = chamfer;
        }
    }
    corners
}

#[derive(Debug)]
pub(crate) struct Look<'a> {
    pub(crate) background: Option<&'a ColorSpec>,
    pub(crate) fill: Option<&'a ColorSpec>,
    pub(crate) stroke: Option<&'a ColorSpec>,
    pub(crate) borders: [(f32, Option<&'a ColorSpec>); 4],
    pub(crate) corners: [Corner; 4],
    pub(crate) style: &'a Modifiers,
    pub(crate) font: Option<Rc<FontStyleInfo>>,
    pub(crate) bound_text: Option<String>,
    pub(crate) shaped: Option<Shaped>,
    pub(crate) pixel: f32,
}

impl<'a> Look<'a> {
    /// Returns the look of `widget` under `style`; `pixel` is how many units of the layout a
    /// pixel of the canvas takes, which font sizes are counted in.
    pub(crate) fn of(
        widget: &'a Widget,
        style: &'a Modifiers,
        pixel: f32,
        font: Option<Rc<FontStyleInfo>>,
    ) -> Self {
        let enabled = style.enable_background.unwrap_or(widget.background_enabled);
        let border = |side: usize| {
            let own = widget.border.get(side);
            let width = style
                .border_widths
                .get(side)
                .copied()
                .flatten()
                .or_else(|| own.map(|own| own.width))
                .unwrap_or(0.0);
            let colour = style
                .border_colours
                .get(side)
                .and_then(Option::as_ref)
                .or_else(|| own.and_then(|own| own.color.as_ref()));
            (width, colour)
        };
        Self {
            background: if enabled {
                style.background.as_ref().or(widget.background.as_ref())
            } else {
                None
            },
            fill: style.fill.as_ref().or(widget.shape.fill.as_ref()),
            stroke: style.stroke.as_ref().or(widget.shape.stroke.as_ref()),
            borders: [border(0), border(1), border(2), border(3)],
            corners: corners_of(widget, style),
            style,
            font,
            bound_text: None,
            shaped: None,
            pixel,
        }
    }
}
