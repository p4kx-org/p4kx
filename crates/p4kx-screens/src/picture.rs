// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::StructValue;

use crate::colour::ColorSpec;
use crate::reader::{child, flag, float, text};

pub(crate) const IMAGE: &str = "BuildingBlocks_WidgetImage";
pub(crate) const CUSTOM_SHAPE: &str = "BuildingBlocks_WidgetCustomShape";
pub(crate) const ICON: &str = "BuildingBlocks_WidgetIcon";
pub(crate) const FOREGROUND_OVERLAY: &str = "BuildingBlocks_WidgetForegroundOverlay";
pub(crate) const BODY_BACKGROUND: &str = "BuildingBlocks_WidgetBodyBackground";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum Fit {
    #[default]
    Fill,
    Contain,
    Cover,
}

impl Fit {
    fn parse(name: Option<&str>) -> Self {
        match name {
            Some("Contain") => Self::Contain,
            Some("Cover") => Self::Cover,
            _ => Self::Fill,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum VectorFit {
    #[default]
    Contain,
    StretchXY,
    StretchX,
    StretchY,
    Center,
}

impl VectorFit {
    pub(crate) fn parse(name: &str) -> Option<Self> {
        Some(match name {
            "Contain" => Self::Contain,
            "StretchXY" => Self::StretchXY,
            "StretchX" => Self::StretchX,
            "StretchY" => Self::StretchY,
            "Center" => Self::Center,
            _ => return None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct NineSlice {
    pub(crate) left: f32,
    pub(crate) right: f32,
    pub(crate) top: f32,
    pub(crate) bottom: f32,
    pub(crate) scale: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VectorSpec {
    pub(crate) path: String,
    pub(crate) fit: VectorFit,
    pub(crate) flip: [bool; 2],
    pub(crate) overlay: bool,
    pub(crate) colour: Option<ColorSpec>,
    pub(crate) place: [f32; 2],
    pub(crate) nine_slice: Option<NineSlice>,
    pub(crate) nine_slice_enabled: bool,
    pub(crate) visible: bool,
}

impl Default for VectorSpec {
    fn default() -> Self {
        Self {
            path: String::new(),
            fit: VectorFit::Contain,
            flip: [false; 2],
            overlay: true,
            colour: None,
            place: [0.0; 2],
            nine_slice: None,
            nine_slice_enabled: false,
            visible: true,
        }
    }
}

impl VectorSpec {
    fn parse(source: &StructValue<'_>, path: String) -> Self {
        let place = |name| float(source, name).unwrap_or(0.0);
        let rect = child(source, "nineSliceRect");
        let edge = |name, default| rect.and_then(|rect| float(rect, name)).unwrap_or(default);
        Self {
            path,
            fit: text(source, "scalingBehavior")
                .and_then(VectorFit::parse)
                .unwrap_or_default(),
            flip: [
                flag(source, "flipHorizontal").unwrap_or(false),
                flag(source, "flipVertical").unwrap_or(false),
            ],
            overlay: flag(source, "enableColorOverlay").unwrap_or(true),
            colour: source.field("color").and_then(ColorSpec::parse),
            place: [place("containPositionX"), place("containPositionY")],
            nine_slice: Some(NineSlice {
                left: edge("left", 0.25),
                right: edge("right", 0.75),
                top: edge("top", 0.25),
                bottom: edge("bottom", 0.75),
                scale: float(source, "nineSliceScale").unwrap_or(1.0),
            }),
            nine_slice_enabled: flag(source, "enableNineSliceRect").unwrap_or(false),
            visible: flag(source, "renderShape").unwrap_or(true),
        }
    }
}

const ICON_FOLDER: &str = "UI/Textures/Vector/General/ModularKit/Widgets/IconWidget";

/// Returns the file of an icon preset, named in snake case in the folder of the icon widget, as
/// `ArrowCaratLeft` is `arrow_carat_left.svg`.
pub(crate) fn icon_preset_path(preset: &str) -> String {
    let mut name = String::with_capacity(preset.len() + 4);
    for (at, letter) in preset.chars().enumerate() {
        if letter.is_ascii_uppercase() {
            if at > 0 {
                name.push('_');
            }
            name.push(letter.to_ascii_lowercase());
        } else {
            name.push(letter);
        }
    }
    format!("{ICON_FOLDER}/{name}.svg")
}

/// What a widget shows as a texture, a vector graphic or an icon.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Picture {
    pub(crate) texture: String,
    pub(crate) runtime_source: Option<String>,
    pub(crate) fit: Fit,
    pub(crate) flip: [bool; 2],
    pub(crate) place: [f32; 2],
    pub(crate) colour: Option<ColorSpec>,
    pub(crate) vector: VectorSpec,
    pub(crate) has_svg_fill: bool,
    pub(crate) icon_preset: Option<String>,
    pub(crate) url: String,
}

pub(crate) const RUNTIME_IMAGE_SOURCE: &str = "imageSource";

impl Picture {
    pub(crate) fn parse(structure: &StructValue<'_>) -> Self {
        let icon = child(structure, "iconProperties");
        let shown = icon.is_none_or(|icon| flag(icon, "show").unwrap_or(true));
        let custom = icon
            .filter(|_| shown)
            .and_then(|icon| text(icon, "customIcon"))
            .filter(|path| !path.is_empty());
        let preset = icon
            .filter(|_| shown)
            .and_then(|icon| text(icon, "iconPreset"))
            .filter(|preset| !matches!(*preset, "" | "_None"));
        let own = text(structure, "svgPath");
        let svg_fill = child(structure, "svgFill");
        let fill_path = svg_fill
            .and_then(|fill| text(fill, "svgPath"))
            .unwrap_or_default();
        let vector = match (own, svg_fill) {
            (Some(path), _) => VectorSpec::parse(structure, path.to_owned()),
            (None, Some(fill)) => VectorSpec::parse(fill, custom.unwrap_or(fill_path).to_owned()),
            (None, None) => VectorSpec::default(),
        };
        Self {
            texture: text(structure, "imagePath").unwrap_or_default().to_owned(),
            runtime_source: text(structure, RUNTIME_IMAGE_SOURCE)
                .filter(|name| !name.trim().is_empty())
                .map(str::to_owned),
            fit: Fit::parse(text(structure, "imageScalingMethod")),
            flip: [
                flag(structure, "flipH").unwrap_or(false),
                flag(structure, "flipV").unwrap_or(false),
            ],
            place: [
                float(structure, "containPositionX").unwrap_or(0.0),
                float(structure, "containPositionY").unwrap_or(0.0),
            ],
            colour: structure.field("color").and_then(ColorSpec::parse),
            vector,
            has_svg_fill: svg_fill.is_some(),
            icon_preset: preset.map(str::to_owned),
            url: text(structure, "urlOptional")
                .unwrap_or_default()
                .to_owned(),
        }
    }
}
