// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod canvas;
mod circle;
mod display;
mod image;
mod overlay;
mod polygon;
mod runtime;
mod standard;
mod text;
mod unsupported;
mod vector;

pub(crate) use canvas::{is_canvas_widget, nested as nested_canvas};
pub(crate) use runtime::RUNTIME_IMAGE;
pub(crate) use standard::{is_standard, tags_of as standard_tags};

use crate::context::Context;
use crate::look::Look;
use crate::paint::Area;
use crate::picture::{BODY_BACKGROUND, CUSTOM_SHAPE, FOREGROUND_OVERLAY, ICON, IMAGE};
use crate::text::{TEXT, TEXT_FIELD};
use crate::widget::Widget;

const STRUCTURAL: [&str; 7] = [
    "BuildingBlocks_DisplayWidget",
    "BuildingBlocks_WidgetBase",
    "BuildingBlocks_WidgetCard",
    "BuildingBlocks_WidgetContainer",
    "BuildingBlocks_WidgetCanvas",
    "BuildingBlocks_WidgetClone",
    "BuildingBlocks_WidgetList",
];

pub(crate) const CIRCLE: &str = "BuildingBlocks_WidgetCircle";
pub(crate) const POLYGON: &str = "BuildingBlocks_WidgetPolygon";

pub(crate) fn is_rendered(type_name: &str) -> bool {
    STRUCTURAL.contains(&type_name)
        || runtime::is_scene(type_name)
        || runtime::is_silent(type_name)
        || is_standard(type_name)
        || matches!(
            type_name,
            CIRCLE
                | POLYGON
                | TEXT
                | TEXT_FIELD
                | IMAGE
                | CUSTOM_SHAPE
                | ICON
                | FOREGROUND_OVERLAY
                | BODY_BACKGROUND
                | RUNTIME_IMAGE
                | runtime::LINE
        )
}

pub(crate) fn paint(
    cx: &mut Context<'_>,
    widget: &Widget,
    look: &Look<'_>,
    area: Area,
    alpha: f32,
) {
    display::paint(cx, widget, look, area, alpha);
    match widget.type_name.as_str() {
        CIRCLE => circle::paint(cx, widget, look, area, alpha),
        POLYGON => polygon::paint(cx, widget, look, area, alpha),
        TEXT | TEXT_FIELD => text::paint(cx, widget, look, area, alpha),
        IMAGE => image::paint(cx, widget, look, area, alpha),
        CUSTOM_SHAPE | ICON => vector::paint(cx, widget, look, area, alpha),
        FOREGROUND_OVERLAY | BODY_BACKGROUND => overlay::paint(cx, widget, look, area, alpha),
        RUNTIME_IMAGE => runtime::image(cx, widget),
        runtime::LINE => runtime::line(cx, widget),
        name if runtime::is_scene(name) => runtime::scene(cx, widget),
        name if STRUCTURAL.contains(&name) || runtime::is_silent(name) || is_standard(name) => {}
        _ => unsupported::note(cx, widget),
    }
}
