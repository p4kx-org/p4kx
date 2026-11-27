// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Budget, Construct, Limits, ReadError, Span, SyntaxError, SyntaxErrorKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Kind {
    Atom,
    Quoted,
    Equals,
    LeftBrace,
    RightBrace,
    LeftParenthesis,
    RightParenthesis,
    Semicolon,
    Newline,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct Token {
    pub(crate) kind: Kind,
    pub(crate) span: Span,
}

pub(crate) fn lex<E>(
    source: &[u8],
    start: usize,
    limits: Limits,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Token>, ReadError<E>> {
    let mut lexer = Lexer {
        source,
        position: start,
        limits,
        checkpoint,
    };
    let mut tokens = Vec::new();
    while let Some(token) = lexer.next()? {
        if tokens.len() >= limits.tokens {
            return Err(SyntaxError::limit(Budget::Tokens, limits.tokens, token.span).into());
        }
        tokens
            .try_reserve(1)
            .map_err(|_| SyntaxError::new(SyntaxErrorKind::Allocation, token.span))?;
        tokens.push(token);
    }
    Ok(tokens)
}

struct Lexer<'source, 'observer, Checkpoint> {
    source: &'source [u8],
    position: usize,
    limits: Limits,
    checkpoint: &'observer mut Checkpoint,
}

impl<E, Checkpoint: FnMut() -> Result<(), E>> Lexer<'_, '_, Checkpoint> {
    fn next(&mut self) -> Result<Option<Token>, ReadError<E>> {
        while let Some(byte) = self.source.get(self.position).copied() {
            (self.checkpoint)().map_err(ReadError::Interrupted)?;
            let start = self.position;
            if matches!(byte, b' ' | b'\t' | 0x0b | 0x0c) {
                self.position += 1;
                continue;
            }
            if self.starts(b"//") {
                self.line_comment(start)?;
                continue;
            }
            if self.starts(b"/*") {
                self.block_comment(start)?;
                continue;
            }
            let kind = match byte {
                b'"' | b'\'' => {
                    self.quoted(start, byte)?;
                    Kind::Quoted
                }
                b'\r' | b'\n' => {
                    self.newline(byte);
                    Kind::Newline
                }
                b'=' => {
                    self.step(start)?;
                    Kind::Equals
                }
                b'{' => {
                    self.step(start)?;
                    Kind::LeftBrace
                }
                b'}' => {
                    self.step(start)?;
                    Kind::RightBrace
                }
                b'(' => {
                    self.step(start)?;
                    Kind::LeftParenthesis
                }
                b')' => {
                    self.step(start)?;
                    Kind::RightParenthesis
                }
                b';' => {
                    self.step(start)?;
                    Kind::Semicolon
                }
                _ => {
                    self.atom(start)?;
                    Kind::Atom
                }
            };
            return Ok(Some(Token {
                kind,
                span: Span::new(start, self.position),
            }));
        }
        Ok(None)
    }

    fn starts(&self, prefix: &[u8]) -> bool {
        self.source
            .get(self.position..)
            .is_some_and(|tail| tail.starts_with(prefix))
    }

    fn step(&mut self, start: usize) -> Result<(), ReadError<E>> {
        (self.checkpoint)().map_err(ReadError::Interrupted)?;
        self.position += 1;
        if self.position - start > self.limits.token_bytes {
            return Err(SyntaxError::limit(
                Budget::TokenBytes,
                self.limits.token_bytes,
                Span::new(start, self.position),
            )
            .into());
        }
        Ok(())
    }

    fn newline(&mut self, byte: u8) {
        self.position += 1;
        if byte == b'\r' && self.source.get(self.position) == Some(&b'\n') {
            self.position += 1;
        }
    }

    fn quoted(&mut self, start: usize, quote: u8) -> Result<(), ReadError<E>> {
        self.step(start)?;
        let mut escaped = false;
        while let Some(byte) = self.source.get(self.position).copied() {
            self.step(start)?;
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == quote {
                return Ok(());
            }
        }
        Err(SyntaxError::new(
            SyntaxErrorKind::Unterminated(Construct::Quote),
            Span::new(start, self.position),
        )
        .into())
    }

    fn line_comment(&mut self, start: usize) -> Result<(), ReadError<E>> {
        while self
            .source
            .get(self.position)
            .is_some_and(|byte| !matches!(byte, b'\n' | b'\r'))
        {
            self.step(start)?;
        }
        Ok(())
    }

    fn block_comment(&mut self, start: usize) -> Result<(), ReadError<E>> {
        self.step(start)?;
        self.step(start)?;
        while self.source.get(self.position).is_some() {
            if self.starts(b"*/") {
                self.step(start)?;
                self.step(start)?;
                return Ok(());
            }
            self.step(start)?;
        }
        Err(SyntaxError::new(
            SyntaxErrorKind::Unterminated(Construct::Comment),
            Span::new(start, self.position),
        )
        .into())
    }

    fn atom(&mut self, start: usize) -> Result<(), ReadError<E>> {
        while let Some(byte) = self.source.get(self.position).copied() {
            if byte.is_ascii_whitespace()
                || b"\"=}{();".contains(&byte)
                || (byte == b'\'' && !self.word_apostrophe(start))
                || self.starts(b"//")
                || self.starts(b"/*")
            {
                break;
            }
            self.step(start)?;
        }
        Ok(())
    }
    fn word_apostrophe(&self, atom_start: usize) -> bool {
        self.position > atom_start
            && self
                .source
                .get(self.position - 1)
                .is_some_and(u8::is_ascii_alphabetic)
            && self
                .source
                .get(self.position + 1)
                .is_some_and(u8::is_ascii_alphabetic)
    }
}
