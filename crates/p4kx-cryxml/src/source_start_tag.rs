// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use crate::source_syntax::SourceSpanError;

#[derive(Debug, Clone)]
pub(crate) struct StartTag<'input> {
    pub(crate) name: &'input str,
    pub(crate) attributes: AttributeTokens<'input>,
}

pub(crate) struct AttributeToken<'input> {
    pub(crate) name: &'input str,
    pub(crate) value: &'input str,
    pub(crate) position: Range<usize>,
}

impl<'input> StartTag<'input> {
    pub(crate) fn new(
        text: &'input str,
        range: Range<usize>,
        prefix_bytes: usize,
    ) -> Result<Self, SourceSpanError> {
        let offset = range.start + prefix_bytes + 1;
        let source = text
            .get(range)
            .and_then(|element| element.strip_prefix('<'))
            .ok_or(SourceSpanError)?;
        let mut attributes = AttributeTokens {
            source,
            offset,
            position: 0,
            done: false,
        };
        let name = attributes.name()?;
        Ok(Self { name, attributes })
    }
}

#[derive(Debug, Clone)]
pub(crate) struct AttributeTokens<'input> {
    source: &'input str,
    offset: usize,
    position: usize,
    done: bool,
}

impl<'input> AttributeTokens<'input> {
    fn remaining(&self) -> Result<&'input str, SourceSpanError> {
        self.source.get(self.position..).ok_or(SourceSpanError)
    }

    fn space(&mut self) -> Result<(), SourceSpanError> {
        self.position += self
            .remaining()?
            .bytes()
            .take_while(u8::is_ascii_whitespace)
            .count();
        Ok(())
    }

    fn name(&mut self) -> Result<&'input str, SourceSpanError> {
        let remaining = self.remaining()?;
        let end = remaining
            .find(|character: char| {
                character.is_ascii_whitespace() || matches!(character, '=' | '/' | '>')
            })
            .filter(|end| *end != 0)
            .ok_or(SourceSpanError)?;
        self.position += end;
        remaining.get(..end).ok_or(SourceSpanError)
    }

    fn attribute(&mut self) -> Result<Option<AttributeToken<'input>>, SourceSpanError> {
        self.space()?;
        if self.remaining()?.starts_with(['>', '/']) {
            return Ok(None);
        }
        let start = self.offset + self.position;
        let name = self.name()?;
        self.space()?;
        if !self.remaining()?.starts_with('=') {
            return Err(SourceSpanError);
        }
        self.position += 1;
        self.space()?;
        let quote = match self.remaining()?.as_bytes().first() {
            Some(b'\'') => '\'',
            Some(b'"') => '"',
            _ => return Err(SourceSpanError),
        };
        self.position += 1;
        let remaining = self.remaining()?;
        let end = remaining.find(quote).ok_or(SourceSpanError)?;
        let value = remaining.get(..end).ok_or(SourceSpanError)?;
        self.position += end + 1;
        Ok(Some(AttributeToken {
            name,
            value,
            position: start..self.offset + self.position,
        }))
    }
}

impl<'input> Iterator for AttributeTokens<'input> {
    type Item = Result<AttributeToken<'input>, SourceSpanError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.attribute() {
            Ok(Some(attribute)) => Some(Ok(attribute)),
            other => {
                self.done = true;
                other.transpose()
            }
        }
    }
}
