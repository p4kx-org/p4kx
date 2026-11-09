// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use crate::discovery_limits::{DiscoveryBudget, DiscoveryLimit};
use crate::steam_libraries::ConfigError;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum Token<'text> {
    Text(Cow<'text, str>),
    Open,
    Close,
}

pub(crate) struct Tokens<'text, 'budget> {
    text: &'text str,
    position: usize,
    pub(crate) budget: &'budget mut DiscoveryBudget,
}

impl<'text, 'budget> Tokens<'text, 'budget> {
    pub(crate) const fn new(text: &'text str, budget: &'budget mut DiscoveryBudget) -> Self {
        Self {
            text,
            position: 0,
            budget,
        }
    }

    pub(crate) fn next(&mut self) -> Result<Option<Token<'text>>, ConfigError> {
        self.layout();
        let Some(&byte) = self.text.as_bytes().get(self.position) else {
            return Ok(None);
        };
        if !self.budget.admit(DiscoveryLimit::ConfigTokens, 1) {
            return Err(ConfigError::Limit(DiscoveryLimit::ConfigTokens));
        }
        self.position += 1;
        match byte {
            b'{' => Ok(Some(Token::Open)),
            b'}' => Ok(Some(Token::Close)),
            b'"' => self.quoted().map(|text| Some(Token::Text(text))),
            _ => self
                .unquoted()
                .map(|text| Some(Token::Text(Cow::Borrowed(text)))),
        }
    }

    fn layout(&mut self) {
        loop {
            while self
                .text
                .as_bytes()
                .get(self.position)
                .is_some_and(u8::is_ascii_whitespace)
            {
                self.position += 1;
            }
            if self
                .text
                .get(self.position..)
                .is_some_and(|text| text.starts_with("//"))
            {
                while self
                    .text
                    .as_bytes()
                    .get(self.position)
                    .is_some_and(|byte| *byte != b'\n')
                {
                    self.position += 1;
                }
            } else {
                return;
            }
        }
    }

    fn unquoted(&mut self) -> Result<&'text str, ConfigError> {
        let start = self.position - 1;
        while self
            .text
            .as_bytes()
            .get(self.position)
            .is_some_and(|byte| !byte.is_ascii_whitespace() && !matches!(byte, b'{' | b'}' | b'"'))
        {
            self.position += 1;
        }
        self.text
            .get(start..self.position)
            .ok_or(ConfigError::Malformed)
    }

    fn quoted(&mut self) -> Result<Cow<'text, str>, ConfigError> {
        let start = self.position;
        while let Some(&byte) = self.text.as_bytes().get(self.position) {
            if byte == b'"' {
                let text = self
                    .text
                    .get(start..self.position)
                    .ok_or(ConfigError::Malformed)?;
                self.position += 1;
                return Ok(Cow::Borrowed(text));
            }
            if byte == b'\\' {
                return self.escaped(start);
            }
            self.position += 1;
        }
        Err(ConfigError::Malformed)
    }

    fn escaped(&mut self, start: usize) -> Result<Cow<'text, str>, ConfigError> {
        let mut bytes = self
            .text
            .as_bytes()
            .get(start..self.position)
            .ok_or(ConfigError::Malformed)?
            .to_vec();
        while let Some(&byte) = self.text.as_bytes().get(self.position) {
            self.position += 1;
            match byte {
                b'"' => {
                    return String::from_utf8(bytes)
                        .map(Cow::Owned)
                        .map_err(|_| ConfigError::Malformed);
                }
                b'\\' => {
                    let escaped = self
                        .text
                        .as_bytes()
                        .get(self.position)
                        .copied()
                        .ok_or(ConfigError::Malformed)?;
                    self.position += 1;
                    bytes.push(match escaped {
                        b'\\' => b'\\',
                        b'"' => b'"',
                        b'n' => b'\n',
                        b'r' => b'\r',
                        b't' => b'\t',
                        _ => return Err(ConfigError::Malformed),
                    });
                }
                _ => bytes.push(byte),
            }
        }
        Err(ConfigError::Malformed)
    }
}
