// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::fmt::Arguments;

use p4kx_character::{SubMaterial, name_hash};
use p4kx_extraction::SourceLocation;
use p4kx_model::{Material, srgb8_to_linear};

#[derive(Default)]
pub(super) struct ChosenFields {
    position: Option<ChoicePosition>,
    values: Vec<SourceLocation>,
}

struct ChoicePosition {
    material: usize,
    sub: usize,
}

#[derive(Clone, Copy)]
pub(super) struct SelectedColour {
    slot: usize,
    rgb: [u8; 3],
}

pub(super) fn colour_of(sub: &SubMaterial, name: u32) -> Option<SelectedColour> {
    let (slot, colour) = sub
        .colors
        .iter()
        .enumerate()
        .find(|(_, colour)| colour.name == name)?;
    let [red, green, blue, _] = colour.rgba;
    Some(SelectedColour {
        slot,
        rgb: [red, green, blue],
    })
}

impl ChosenFields {
    pub(super) fn new(material: Option<usize>, sub: usize) -> Self {
        Self {
            position: material.map(|material| ChoicePosition { material, sub }),
            values: Vec::new(),
        }
    }

    pub(super) fn colour(&mut self, colour: SelectedColour) -> [f32; 3] {
        if let Some(position) = &self.position {
            for channel in 0..colour.rgb.len() {
                self.values
                    .push(position.field(format_args!("colors/{}/rgba/{channel}", colour.slot)));
            }
        }
        colour.rgb.map(srgb8_to_linear)
    }

    pub(super) fn number(
        &mut self,
        sub: Option<&SubMaterial>,
        material: &Material,
        name: &str,
    ) -> f32 {
        let hash = name_hash(name);
        let source = sub.and_then(|sub| {
            sub.floats
                .iter()
                .enumerate()
                .find(|(_, param)| param.name == hash)
        });
        let value = if let Some((slot, param)) = source {
            if param.value.is_finite()
                && let Some(position) = &self.position
            {
                self.values
                    .push(position.field(format_args!("floats/{slot}/value")));
            }
            Some(param.value)
        } else {
            material.public_param(name).map(|value| value[0])
        };
        value
            .filter(|value| value.is_finite())
            .map_or(0.0, |value| value.clamp(0.0, 1.0))
    }

    pub(super) fn finish(self) -> Option<Vec<SourceLocation>> {
        (!self.values.is_empty()).then_some(self.values)
    }
}

impl ChoicePosition {
    fn field(&self, tail: Arguments<'_>) -> SourceLocation {
        SourceLocation::Subresource {
            namespace: "chf".into(),
            indices: Vec::new(),
            field: format!(
                "/character/materials/{}/sub_materials/{}/{tail}",
                self.material, self.sub
            ),
        }
    }
}
