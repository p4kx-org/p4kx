// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_texture::TextureError;

use crate::ConversionError;

impl ConversionError {
    /// Names the part of a [`TextureError`] about one part, `<texture>.N` or for an alpha tail
    /// `<texture>.a` its part `<texture>.Na`, or wraps it with the texture's name.
    pub(crate) fn of_texture(texture: &str, error: TextureError) -> Self {
        let part = |index: u32| match texture
            .len()
            .checked_sub(2)
            .and_then(|at| texture.split_at_checked(at))
        {
            Some((base, suffix @ (".a" | ".A"))) => {
                format!("{base}.{index}{}", suffix.trim_start_matches('.'))
            }
            _ => format!("{texture}.{index}"),
        };
        match error {
            TextureError::MissingPart { index } => Self::MissingPart { part: part(index) },
            TextureError::TruncatedPart {
                index,
                expected,
                actual,
            } => Self::TruncatedPart {
                part: part(index),
                expected,
                actual,
            },
            TextureError::OversizedPart {
                index,
                expected,
                actual,
            } => Self::OversizedPart {
                part: part(index),
                expected,
                actual,
            },
            TextureError::ReadPart { index, source } => Self::ReadPart {
                part: part(index),
                source,
            },
            source => Self::Texture {
                texture: texture.to_owned(),
                source,
            },
        }
    }
}
