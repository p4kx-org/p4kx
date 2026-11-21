// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::GfxError;
use crate::font::{Font, FontFormat};
use crate::tag::Tag;

const INFO_SMALL_TEXT: u8 = 0x20;
const INFO_ITALIC: u8 = 0x04;
const INFO_BOLD: u8 = 0x02;
const INFO_WIDE_CODES: u8 = 0x01;
const FONT_INFO2: u16 = 62;

/// The name, style and codes a `DefineFontInfo` or `DefineFontInfo2` tag gives a `DefineFont` font.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FontInfo {
    pub(crate) id: u16,
    name: String,
    flags: u8,
    codes: Vec<u16>,
}

impl FontInfo {
    /// Gives `font` the name, style and glyph codes and returns the number of codes written.
    pub(crate) fn apply(self, font: &mut Font) -> usize {
        font.name = self.name;
        font.bold = self.flags & INFO_BOLD != 0;
        font.italic = self.flags & INFO_ITALIC != 0;
        font.small_text = self.flags & INFO_SMALL_TEXT != 0;
        let written = font.glyphs.len().min(self.codes.len());
        for (glyph, code) in font.glyphs.iter_mut().zip(self.codes) {
            glyph.code = Some(code);
        }
        written
    }
}

/// Reads `DefineFontInfo` or `DefineFontInfo2`.
pub(crate) fn read_font_info(tag: &Tag<'_>) -> Result<FontInfo, GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    let name = reader.short_string()?;
    let flags = reader.u8()?;
    if tag.code == FONT_INFO2 {
        reader.u8()?;
    }
    let codes = reader.rest();
    let codes = if flags & INFO_WIDE_CODES != 0 || tag.code == FONT_INFO2 {
        codes
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&pair| u16::from_le_bytes(pair))
            .collect()
    } else {
        codes.iter().copied().map(u16::from).collect()
    };
    Ok(FontInfo {
        id,
        name,
        flags,
        codes,
    })
}

/// Reads the ID and full name of `DefineFontName`.
pub(crate) fn read_font_name(tag: &Tag<'_>) -> Result<(u16, String), GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    Ok((id, reader.c_string()?.into_owned()))
}

/// Reads the ID, style and name of `DefineFont4`; its CFF data is not read.
pub(crate) fn read_font4(tag: &Tag<'_>) -> Result<Font, GfxError> {
    let mut reader = tag.reader();
    let id = reader.u16()?;
    let flags = reader.u8()?;
    Ok(Font {
        id,
        format: FontFormat::DefineFont4,
        name: reader.c_string()?.into_owned(),
        full_name: None,
        export_name: None,
        import_url: None,
        bold: flags & 0x01 != 0,
        italic: flags & 0x02 != 0,
        small_text: false,
        glyph_count: 0,
        glyphs: Vec::new(),
        layout: None,
    })
}
