// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use serde::Serialize;

use crate::screen_asset_inputs::WidgetInput;
use crate::widget::Widget;

const MAX_EXAMPLES: usize = 3;
const MAX_MISSING_TEXTURES: usize = 256;

/// An element type or feature the render left out or approximated, with how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Unsupported {
    /// The element type, e.g. `WidgetText`, without the `BuildingBlocks_` prefix.
    pub element: String,
    /// What was left out or approximated.
    pub reason: String,
    /// How many widgets it concerns.
    pub count: usize,
    /// Names of up to three of those widgets.
    pub examples: Vec<String>,
}

/// A game variable that was set for the render.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct VariableInForce {
    /// The name of the variable.
    pub name: String,
    /// The value it was set to, as it was given.
    pub value: String,
    /// Whether an operation of a canvas of the screen reads a variable of that name; `false` for
    /// a name that the screen does not know.
    pub used: bool,
}

/// Why a widget's requested content was unavailable.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[non_exhaustive]
#[serde(rename_all = "snake_case")]
pub enum ContentOmissionCode {
    /// The source field names no target or variable.
    SourceMissing,
    /// The source names a render target that the game renders.
    RuntimeTarget,
    /// The source names a list array that has no static rows.
    RuntimeArray,
    /// The caller supplied no value for a text variable.
    VariableMissing,
    /// The field binding is outside the supported renderer.
    UnsupportedField,
    /// The operation supplying the field could not be evaluated.
    UnresolvedInput,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum OmissionInput {
    WidgetField(&'static str),
    Binding {
        operation: usize,
        variable: Option<usize>,
    },
}

/// The widget and source field affected by a runtime or binding omission.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[non_exhaustive]
pub struct ContentOmission {
    /// The element type without the `BuildingBlocks_` prefix.
    pub element: String,
    /// The widget name in the canvas.
    pub widget: String,
    /// The widget's original canvas Record and position.
    pub widget_input: WidgetInput,
    /// The authored source field or the field selected by the binding.
    pub field: String,
    /// The authored target identifier or direct variable path, if present.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// Why the requested content was unavailable.
    pub code: ContentOmissionCode,
    #[serde(skip)]
    pub(crate) input: OmissionInput,
}

/// What a render drew and what it left out.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct Report {
    /// The widgets visited, every nested canvas included.
    pub widgets: usize,
    /// The shapes drawn, every placement of a glyph counted.
    pub shapes: usize,
    /// The text widgets whose text was drawn.
    pub texts_drawn: usize,
    /// The text widgets whose text was not drawn: no fonts were given, the text is empty or filled
    /// in by the game at run time, the font or colour is missing, or a limit was reached.
    pub texts_left_out: usize,
    /// The widgets whose texture or vector graphic was drawn.
    pub images_drawn: usize,
    /// The widgets whose texture or vector graphic was not drawn: no image sources were given,
    /// the file is missing, unreadable or beyond a limit, or the widget names none.
    pub images_left_out: usize,
    /// The canvases composed, the screen's own first.
    pub canvases: usize,
    /// The element types and features left out or approximated.
    pub unsupported: Vec<Unsupported>,
    /// Runtime content omissions, once per original widget, authored source location and reason code.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub content_omissions: Vec<ContentOmission>,
    /// Record paths of nested canvases that could not be read.
    pub missing_canvases: Vec<String>,
    /// The Archive paths of textures that widgets name and the Archive does not hold or cannot
    /// read, each once, in the order they were met; at most 256.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing_textures: Vec<String>,
    /// Whether a limit stopped the composition before the whole screen was drawn.
    pub truncated: bool,
    /// The game variables that were set, the last setting of each name; empty at rest.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub variables: Vec<VariableInForce>,
}

impl Report {
    pub(crate) fn content_omission(
        &mut self,
        widget: &Widget,
        field: &str,
        source: Option<&str>,
        code: ContentOmissionCode,
        input: OmissionInput,
    ) {
        let element = widget
            .type_name
            .strip_prefix("BuildingBlocks_")
            .unwrap_or(&widget.type_name);
        self.content_omissions.push(ContentOmission {
            element: element.to_owned(),
            widget: widget.name.clone(),
            widget_input: widget.input,
            field: field.to_owned(),
            source: source.map(str::to_owned),
            code,
            input,
        });
    }

    pub(crate) fn note(&mut self, element: &str, reason: &str, name: &str) {
        let element = element.strip_prefix("BuildingBlocks_").unwrap_or(element);
        if let Some(entry) = self
            .unsupported
            .iter_mut()
            .find(|entry| entry.element == element && entry.reason == reason)
        {
            entry.count += 1;
            if entry.examples.len() < MAX_EXAMPLES && !entry.examples.iter().any(|n| n == name) {
                entry.examples.push(name.to_owned());
            }
            return;
        }
        self.unsupported.push(Unsupported {
            element: element.to_owned(),
            reason: reason.to_owned(),
            count: 1,
            examples: vec![name.to_owned()],
        });
    }

    /// Lists the Archive path `path` of a texture that could not be read, once.
    pub(crate) fn missing_texture(&mut self, path: &str) {
        if self.missing_textures.len() < MAX_MISSING_TEXTURES
            && !self.missing_textures.iter().any(|known| known == path)
        {
            self.missing_textures.push(path.to_owned());
        }
    }

    /// Returns the names of the variables that were set and that no canvas of the screen reads.
    pub fn unknown_variables(&self) -> impl Iterator<Item = &str> {
        self.variables
            .iter()
            .filter(|variable| !variable.used)
            .map(|variable| variable.name.as_str())
    }

    /// Returns whether the whole screen was drawn with every effect.
    #[must_use]
    pub const fn is_complete(&self) -> bool {
        self.unsupported.is_empty() && self.missing_canvases.is_empty() && !self.truncated
    }
}
