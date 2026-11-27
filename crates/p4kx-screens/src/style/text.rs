// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{Guid, StructValue};

use super::preset::PresetFill;
use crate::reader::{reference, text};
use crate::text::{Case, Horizontal, Vertical};

const FONT_STYLE: &str = "BuildingBlocks_FieldModifierRecordRefTypeFontStyleRecord";
const ENUM_PREFIX: &str = "BuildingBlocks_FieldModifierEnumeratedType";

#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct TextModifiers {
    pub(crate) font_style: Option<Guid>,
    pub(crate) font_size: Option<f32>,
    pub(crate) letter_spacing: Option<f32>,
    pub(crate) line_spacing: Option<f32>,
    pub(crate) wrap: Option<bool>,
    pub(crate) auto_size: Option<bool>,
    pub(crate) horizontal: Option<Horizontal>,
    pub(crate) vertical: Option<Vertical>,
    pub(crate) case: Option<Case>,
    pub(crate) text: Option<String>,
    pub(crate) preset_fill: Option<PresetFill>,
}

impl TextModifiers {
    pub(crate) fn number(&mut self, field: &str, value: Option<f32>) {
        let value = value.filter(|value| value.is_finite());
        match field {
            "FontSize" => self.font_size = value,
            "LetterSpacing" => self.letter_spacing = value,
            "LineSpacing" => self.line_spacing = value,
            _ => {}
        }
    }

    pub(crate) fn boolean(&mut self, field: &str, value: Option<bool>) {
        match field {
            "WordWrap" => self.wrap = value,
            "AutoFontSize" => self.auto_size = value,
            _ => {}
        }
    }

    pub(crate) fn localised(&mut self, field: &str, value: Option<&str>) {
        if field == "Text" {
            self.text = value.map(str::to_owned);
        }
    }

    pub(crate) fn typed(&mut self, inner: &StructValue<'_>) {
        let name = inner.type_name();
        if name == FONT_STYLE {
            self.font_style = reference(inner, "value").map(p4kx_dataforge::Reference::guid);
            return;
        }
        let Some(kind) = name.strip_prefix(ENUM_PREFIX) else {
            return;
        };
        let value = text(inner, "value");
        match kind {
            "HorizontalAlignment" | "ModularKitHorizontalAlignment" => {
                self.horizontal = value.and_then(Horizontal::parse);
            }
            "VerticalAlignment" | "ModularKitVerticalAlignment" => {
                self.vertical = value.and_then(Vertical::parse);
            }
            "CaseModifier" | "ModularKitCaseModifier" => {
                self.case = value.and_then(Case::parse);
            }
            _ => {}
        }
    }

    pub(crate) fn merge(&mut self, from: &Self) {
        fn set<T: Clone>(slot: &mut Option<T>, value: Option<&T>) {
            if let Some(value) = value {
                *slot = Some(value.clone());
            }
        }
        set(&mut self.font_style, from.font_style.as_ref());
        set(&mut self.font_size, from.font_size.as_ref());
        set(&mut self.letter_spacing, from.letter_spacing.as_ref());
        set(&mut self.line_spacing, from.line_spacing.as_ref());
        set(&mut self.wrap, from.wrap.as_ref());
        set(&mut self.auto_size, from.auto_size.as_ref());
        set(&mut self.horizontal, from.horizontal.as_ref());
        set(&mut self.vertical, from.vertical.as_ref());
        set(&mut self.case, from.case.as_ref());
        set(&mut self.text, from.text.as_ref());
        set(&mut self.preset_fill, from.preset_fill.as_ref());
    }
}
