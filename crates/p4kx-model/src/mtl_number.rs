// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(crate) fn scalar(value: &str) -> Option<f32> {
    value
        .trim()
        .parse::<f32>()
        .ok()
        .filter(|value| value.is_finite())
}

pub(crate) fn vector(value: &str) -> Option<[f32; 3]> {
    let mut parts = value.split(',').map(scalar);
    let colour = [parts.next()??, parts.next()??, parts.next()??];
    parts.next().is_none().then_some(colour)
}
