// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Read;

use p4kx_texture::{DdsHeader, DdsRawHeader, MAX_HEADER_LEN, TextureError};

use crate::ConversionError;

pub(crate) struct TextureHeaders {
    pub(crate) raw: DdsRawHeader,
    pub(crate) normalized: DdsHeader,
}

pub(crate) fn read(reader: impl Read, name: &str) -> Result<TextureHeaders, ConversionError> {
    let mut start = Vec::with_capacity(MAX_HEADER_LEN);
    reader
        .take(MAX_HEADER_LEN as u64)
        .read_to_end(&mut start)
        .map_err(|source| ConversionError::of_texture(name, TextureError::ReadHeader(source)))?;
    let raw =
        DdsRawHeader::parse(&start).map_err(|error| ConversionError::of_texture(name, error))?;
    let normalized =
        DdsHeader::from_raw(&raw).map_err(|error| ConversionError::of_texture(name, error))?;
    Ok(TextureHeaders { raw, normalized })
}
