// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::StructValue;

use crate::picture::{NineSlice, VectorFit};
use crate::reader::text;

/// The cascade modifiers of textures and vector graphics.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct PictureModifiers {
    pub(crate) image_path: Option<String>,
    pub(crate) svg_path: Option<String>,
    pub(crate) svg_fit: Option<VectorFit>,
    pub(crate) overlay: Option<bool>,
    pub(crate) image_flip: [Option<bool>; 2],
    pub(crate) svg_flip: [Option<bool>; 2],
    pub(crate) nine_slice: Option<bool>,
    nine_slice_edges: [Option<f32>; 4],
}

impl PictureModifiers {
    pub(crate) fn typed(&mut self, field: &StructValue<'_>) {
        if field.type_name() == "BuildingBlocks_FieldModifierEnumeratedTypeSvgScalingBehavior" {
            self.svg_fit = text(field, "value").and_then(VectorFit::parse);
        }
    }

    pub(crate) fn number(&mut self, field: &str, value: Option<f32>) {
        let [top, right, bottom, left] = &mut self.nine_slice_edges;
        let edge = match field {
            "NineSliceTop" => top,
            "NineSliceRight" => right,
            "NineSliceBottom" => bottom,
            "NineSliceLeft" => left,
            _ => return,
        };
        *edge = value;
    }

    pub(crate) fn slice(&self, own: NineSlice) -> NineSlice {
        let [top, right, bottom, left] = self.nine_slice_edges;
        NineSlice {
            top: top.unwrap_or(own.top),
            right: right.unwrap_or(own.right),
            bottom: bottom.unwrap_or(own.bottom),
            left: left.unwrap_or(own.left),
            scale: own.scale,
        }
    }

    pub(crate) fn string(&mut self, field: &str, value: Option<&str>) {
        let value = value.map(str::to_owned);
        match field {
            "ImagePath" => self.image_path = value,
            "SvgPath" => self.svg_path = value,
            _ => {}
        }
    }

    pub(crate) fn boolean(&mut self, field: &str, value: Option<bool>) {
        match field {
            "EnableColorOverlay" => self.overlay = value,
            "EnableNineSliceRect" => self.nine_slice = value,
            "ImageFlipHorizontal" => self.image_flip[0] = value,
            "ImageFlipVertical" => self.image_flip[1] = value,
            "SvgFlipHorizontal" => self.svg_flip[0] = value,
            "SvgFlipVertical" => self.svg_flip[1] = value,
            _ => {}
        }
    }

    pub(crate) fn merge(&mut self, from: &Self) {
        fn set<T: Clone>(slot: &mut Option<T>, value: Option<&T>) {
            if let Some(value) = value {
                *slot = Some(value.clone());
            }
        }
        set(&mut self.image_path, from.image_path.as_ref());
        set(&mut self.svg_path, from.svg_path.as_ref());
        set(&mut self.svg_fit, from.svg_fit.as_ref());
        set(&mut self.overlay, from.overlay.as_ref());
        set(&mut self.nine_slice, from.nine_slice.as_ref());
        for (slot, value) in self.nine_slice_edges.iter_mut().zip(&from.nine_slice_edges) {
            set(slot, value.as_ref());
        }
        for (slot, value) in self.image_flip.iter_mut().zip(&from.image_flip) {
            set(slot, value.as_ref());
        }
        for (slot, value) in self.svg_flip.iter_mut().zip(&from.svg_flip) {
            set(slot, value.as_ref());
        }
    }
}
