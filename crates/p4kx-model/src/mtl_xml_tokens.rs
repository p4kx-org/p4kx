// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::MaterialError;

pub(crate) struct AttributeToken<'input> {
    pub(crate) name: &'input str,
    pub(crate) value: &'input str,
}

pub(crate) struct StartTag<'input> {
    pub(crate) name: &'input str,
    pub(crate) attributes: Vec<AttributeToken<'input>>,
}

/// Extracts lexical spans from an already validated plain-XML start tag.
pub(crate) fn start_tag(text: &str, range: Range<usize>) -> Result<StartTag<'_>, MaterialError> {
    let source = text.get(range).ok_or(MaterialError::SourceSpan)?;
    let Some(source) = source.strip_prefix('<') else {
        return Err(MaterialError::SourceSpan);
    };
    let mut cursor = Cursor {
        source,
        position: 0,
    };
    let name = cursor.name()?;
    let mut attributes = Vec::new();
    loop {
        cursor.space()?;
        if cursor.remaining()?.starts_with(['>', '/']) {
            return Ok(StartTag { name, attributes });
        }
        attributes.push(cursor.attribute()?);
    }
}

struct Cursor<'input> {
    source: &'input str,
    position: usize,
}

impl<'input> Cursor<'input> {
    fn remaining(&self) -> Result<&'input str, MaterialError> {
        self.source
            .get(self.position..)
            .ok_or(MaterialError::SourceSpan)
    }

    fn space(&mut self) -> Result<(), MaterialError> {
        let remaining = self.remaining()?;
        self.position += remaining
            .bytes()
            .take_while(u8::is_ascii_whitespace)
            .count();
        Ok(())
    }

    fn name(&mut self) -> Result<&'input str, MaterialError> {
        let remaining = self.remaining()?;
        let end = remaining
            .find(|character: char| {
                character.is_ascii_whitespace() || matches!(character, '=' | '/' | '>')
            })
            .ok_or(MaterialError::SourceSpan)?;
        if end == 0 {
            return Err(MaterialError::SourceSpan);
        }
        self.position += end;
        remaining.get(..end).ok_or(MaterialError::SourceSpan)
    }

    fn attribute(&mut self) -> Result<AttributeToken<'input>, MaterialError> {
        let name = self.name()?;
        self.space()?;
        if !self.remaining()?.starts_with('=') {
            return Err(MaterialError::SourceSpan);
        }
        self.position += 1;
        self.space()?;
        let quote = match self.remaining()?.as_bytes().first() {
            Some(b'\'') => '\'',
            Some(b'"') => '"',
            _ => return Err(MaterialError::SourceSpan),
        };
        self.position += 1;
        let remaining = self.remaining()?;
        let end = remaining.find(quote).ok_or(MaterialError::SourceSpan)?;
        let value = remaining.get(..end).ok_or(MaterialError::SourceSpan)?;
        self.position += end + 1;
        Ok(AttributeToken { name, value })
    }
}
