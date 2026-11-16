// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, Guid};

use crate::button_component::ButtonFamily;
use crate::canvas::find_canvas;
use crate::style::TagPaths;
use crate::widget::Widget;
use crate::widget_canvas_ref::WidgetCanvasRef;

const FOLDER: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/widgets/";
const TAG_FOLDER: &str = "UI.Style._Standard.ModularKit.Widgets";
const GENERAL_TAG: &str = "UI.Style._Standard.ModularKit.Components.General-Button-Primary";
pub(crate) const GENERAL_CANVAS: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/components/buttongeneralcomponentstandard";
const SECONDARY_TAG: &str = "UI.Style._Standard.ModularKit.Components.General-Button-Secondary";
pub(crate) const SECONDARY_CANVAS: &str = "libs/foundry/records/ui/buildingblocks/modularkit/standard/components/buttongeneralsecondarycomponentstandard";

pub(crate) const SEPARATOR: &str = "BuildingBlocks_WidgetSeparator";

const DIRECTIONS: [&str; 2] = ["horizontal", "vertical"];
const STYLES: [&str; 3] = ["primary", "secondary", "tertiary"];

/// Returns whether the look of `type_name` is kept in a canvas of the `ModularKit` standard
/// widgets.
pub(crate) fn is_standard(type_name: &str) -> bool {
    type_name == SEPARATOR || ButtonFamily::of_type(type_name).is_some()
}

fn variant(widget: &Widget) -> Option<[String; 2]> {
    if widget.type_name != SEPARATOR {
        return None;
    }
    let direction = widget.kit.direction.to_ascii_lowercase();
    let style = widget.kit.style.to_ascii_lowercase();
    (DIRECTIONS.contains(&direction.as_str()) && STYLES.contains(&style.as_str()))
        .then_some([direction, style])
}

/// Returns the canvas that holds the look of a standard widget, named by its direction and
/// style, when the Archive has it.
pub(crate) fn canvas_of(widget: &Widget, forge: &DataForge<'_>) -> Option<Guid> {
    if let Some(button) = &widget.button {
        let template = match button.family() {
            ButtonFamily::General if matches!(widget.canvas, WidgetCanvasRef::Absent) => {
                GENERAL_CANVAS
            }
            ButtonFamily::General => return None,
            ButtonFamily::Secondary => SECONDARY_CANVAS,
        };
        return find_canvas(forge, template).ok();
    }
    let [direction, style] = variant(widget)?;
    find_canvas(
        forge,
        &format!("{FOLDER}{direction}separator{style}widgetstandard"),
    )
    .ok()
}

/// Returns the standard component and widget tags selected by their template styles.
pub(crate) fn tags_of(widget: &Widget, paths: &TagPaths) -> Vec<Guid> {
    if let Some(button) = &widget.button {
        let name = match button.family() {
            ButtonFamily::General => GENERAL_TAG,
            ButtonFamily::Secondary => SECONDARY_TAG,
        };
        return paths.guid_of(name).into_iter().collect();
    }
    let Some(parts) = variant(widget) else {
        return Vec::new();
    };
    parts
        .iter()
        .filter_map(|part| paths.guid_of(&format!("{TAG_FOLDER}.separator.{part}")))
        .collect()
}
