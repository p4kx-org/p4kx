// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::Material;

pub(crate) enum PublicValue {
    Number(f32),
    Colour([f32; 3]),
}

pub(crate) struct PublicParameter {
    pub(crate) value: Option<PublicValue>,
    pub(crate) index: Option<usize>,
}

impl PublicParameter {
    pub(crate) fn components(self) -> Option<[f32; 3]> {
        match self.value? {
            PublicValue::Number(first) => Some([first, 0.0, 0.0]),
            PublicValue::Colour(colour) => Some(colour),
        }
    }
}

pub(crate) fn read(material: &Material, name: &str) -> PublicParameter {
    let found = material
        .public_params
        .iter()
        .enumerate()
        .find(|(_, (key, _))| key.eq_ignore_ascii_case(name));
    match found {
        Some((index, (_, text))) => PublicParameter {
            value: parse(text),
            index: Some(index),
        },
        None => PublicParameter {
            value: None,
            index: None,
        },
    }
}

fn parse(text: &str) -> Option<PublicValue> {
    let number = |part: &str| {
        part.trim()
            .parse::<f32>()
            .ok()
            .filter(|value| value.is_finite())
    };
    let mut parts = text.split(',');
    let first = number(parts.next()?)?;
    match (parts.next(), parts.next(), parts.next()) {
        (None, ..) => Some(PublicValue::Number(first)),
        (Some(second), Some(third), None) => Some(PublicValue::Colour([
            first,
            number(second)?,
            number(third)?,
        ])),
        _ => None,
    }
}
