// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

/// The original widget array in a canvas Record.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum WidgetSection {
    /// The canvas's scene array.
    Scene,
    /// The canvas's library array.
    Library,
}

impl WidgetSection {
    /// Returns the original canvas field holding this widget array.
    #[must_use]
    pub const fn field_name(self) -> &'static str {
        match self {
            Self::Scene => "scene",
            Self::Library => "library",
        }
    }
}

/// An actual original widget contributing to the listing.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct WidgetInput {
    /// The physical canvas Record position.
    pub record_index: u32,
    /// The original array holding the widget.
    pub section: WidgetSection,
    /// The widget's original position in that array before projection.
    pub slot: usize,
}

/// An authored widget field that supplied a listed asset path or localization key.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct WidgetAssetInput {
    /// The original widget that owns the value.
    pub widget: WidgetInput,
    /// The field's canonical preorder position within the widget.
    pub ordinal: usize,
}

/// The admitted canvas and the original direct widget Reference that selected it.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct CanvasRouteInput {
    /// The physical admitted canvas Record position.
    pub record_index: u32,
    /// The selecting widget, when its original direct Reference was retained.
    pub through_widget: Option<WidgetInput>,
}

/// Original inspected sources, aligned to the lists and widget groups in one screen row.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ScreenAssetInputs {
    /// Physical source Record of each listed canvas path.
    pub canvases: Vec<u32>,
    /// Actual admitted canvas routes in traversal order.
    pub canvas_routes: Vec<CanvasRouteInput>,
    /// Original field of each listed texture path.
    pub textures: Vec<WidgetAssetInput>,
    /// Original field of each listed vector path.
    pub vectors: Vec<WidgetAssetInput>,
    /// Original field of each listed movie path.
    pub movies: Vec<WidgetAssetInput>,
    /// Original field of each listed localization key.
    pub localisation_keys: Vec<WidgetAssetInput>,
    /// All original widget declarations counted in each displayed widget group.
    pub elements: Vec<Vec<WidgetInput>>,
}
