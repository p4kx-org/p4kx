// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::PathBuf;

use crate::discovery_limits::{DiscoveryBudget, DiscoveryLimit};
use crate::steam_tokens::{Token, Tokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ConfigError {
    Malformed,
    Limit(DiscoveryLimit),
}

pub(crate) fn parse(text: &str, budget: &mut DiscoveryBudget) -> Result<Vec<PathBuf>, ConfigError> {
    let text = text.strip_prefix('\u{feff}').unwrap_or(text);
    let mut parser = Libraries {
        tokens: Tokens::new(text, budget),
        roots: Vec::new(),
    };
    match parser.tokens.next()? {
        Some(Token::Text(name)) if name.eq_ignore_ascii_case("libraryfolders") => {}
        _ => return Err(ConfigError::Malformed),
    }
    if parser.tokens.next()? != Some(Token::Open) {
        return Err(ConfigError::Malformed);
    }
    parser.depth(1)?;
    parser.folders()?;
    if parser.tokens.next()?.is_some() {
        return Err(ConfigError::Malformed);
    }
    Ok(parser.roots)
}

struct Libraries<'text, 'budget> {
    tokens: Tokens<'text, 'budget>,
    roots: Vec<PathBuf>,
}

impl Libraries<'_, '_> {
    const fn depth(&self, depth: usize) -> Result<(), ConfigError> {
        if depth > self.tokens.budget.limits.config_depth {
            Err(ConfigError::Limit(DiscoveryLimit::ConfigDepth))
        } else {
            Ok(())
        }
    }

    fn folders(&mut self) -> Result<(), ConfigError> {
        loop {
            let key = match self.tokens.next()? {
                Some(Token::Close) => return Ok(()),
                Some(Token::Text(key)) => key,
                _ => return Err(ConfigError::Malformed),
            };
            let numeric = !key.is_empty() && key.bytes().all(|byte| byte.is_ascii_digit());
            match self.tokens.next()? {
                Some(Token::Text(path)) if numeric => self.path(path.as_ref())?,
                Some(Token::Text(_)) => {}
                Some(Token::Open) if numeric => self.library(2)?,
                Some(Token::Open) => self.skip(2)?,
                _ => return Err(ConfigError::Malformed),
            }
        }
    }

    fn library(&mut self, depth: usize) -> Result<(), ConfigError> {
        self.depth(depth)?;
        loop {
            let key = match self.tokens.next()? {
                Some(Token::Close) => return Ok(()),
                Some(Token::Text(key)) => key,
                _ => return Err(ConfigError::Malformed),
            };
            match self.tokens.next()? {
                Some(Token::Text(path)) if key.eq_ignore_ascii_case("path") => {
                    self.path(path.as_ref())?;
                }
                Some(Token::Text(_)) => {}
                Some(Token::Open) => self.skip(depth + 1)?,
                _ => return Err(ConfigError::Malformed),
            }
        }
    }

    fn skip(&mut self, depth: usize) -> Result<(), ConfigError> {
        self.depth(depth)?;
        loop {
            match self.tokens.next()? {
                Some(Token::Close) => return Ok(()),
                Some(Token::Text(_)) => {}
                _ => return Err(ConfigError::Malformed),
            }
            match self.tokens.next()? {
                Some(Token::Text(_)) => {}
                Some(Token::Open) => self.skip(depth + 1)?,
                _ => return Err(ConfigError::Malformed),
            }
        }
    }

    fn path(&mut self, text: &str) -> Result<(), ConfigError> {
        let path = PathBuf::from(text);
        if !path.is_absolute() {
            return Err(ConfigError::Malformed);
        }
        if !self.roots.contains(&path) {
            if !self.tokens.budget.admit(DiscoveryLimit::Roots, 1) {
                return Err(ConfigError::Limit(DiscoveryLimit::Roots));
            }
            self.roots.push(path);
        }
        Ok(())
    }
}
