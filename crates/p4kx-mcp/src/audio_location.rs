// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;

use p4kx_assembly::audio::WemLocation;

use crate::audio_shapes::LocationShape;
use crate::budget::Budget;
use crate::error::ToolError;

/// Copies the physical candidate location without imposing an output-response budget.
pub(crate) fn location(location: &WemLocation<'_>) -> Option<LocationShape> {
    match copy_with(location, |text| Ok::<_, Infallible>(text.to_owned())) {
        Ok(location) => location,
        Err(never) => match never {},
    }
}

pub(crate) fn bounded(
    location: &WemLocation<'_>,
    text: &mut Budget,
) -> Result<Option<LocationShape>, ToolError> {
    copy_with(location, |value| text.text(value))
}

fn copy_with<Failure>(
    location: &WemLocation<'_>,
    mut text: impl FnMut(&str) -> Result<String, Failure>,
) -> Result<Option<LocationShape>, Failure> {
    Ok(Some(match location {
        WemLocation::Soundbank {
            soundbank,
            offset,
            len,
        } => LocationShape {
            kind: "soundbank",
            path: text(soundbank.path())?,
            language: soundbank.language().map(&mut text).transpose()?,
            offset: Some(*offset),
            len: *len,
        },
        WemLocation::Entry {
            path,
            language,
            len,
            ..
        } => LocationShape {
            kind: "entry",
            path: text(path)?,
            language: language.map(&mut text).transpose()?,
            offset: None,
            len: *len,
        },
        _ => return Ok(None),
    }))
}
