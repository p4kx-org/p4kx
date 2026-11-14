// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::lexer::{Kind, Token};
use crate::{
    Budget, Construct, Declaration, Form, Limits, ReadError, Span, SyntaxError, SyntaxErrorKind,
};

pub(crate) fn parse<E>(
    source: &str,
    tokens: &[Token],
    limits: Limits,
    checkpoint: &mut impl FnMut() -> Result<(), E>,
) -> Result<Vec<Declaration>, ReadError<E>> {
    let mut parser = Parser {
        source,
        tokens,
        position: 0,
        limits,
        checkpoint,
        declarations: 0,
        properties: 0,
    };
    Ok(parser.scope(0, None)?.children)
}

struct Scope {
    children: Vec<Declaration>,
    end: usize,
}

struct Parser<'source, 'tokens, 'observer, Checkpoint> {
    source: &'source str,
    tokens: &'tokens [Token],
    position: usize,
    limits: Limits,
    checkpoint: &'observer mut Checkpoint,
    declarations: usize,
    properties: usize,
}

impl<E, Checkpoint: FnMut() -> Result<(), E>> Parser<'_, '_, '_, Checkpoint> {
    fn scope(&mut self, depth: usize, open: Option<Token>) -> Result<Scope, ReadError<E>> {
        let mut children = Vec::new();
        while let Some(token) = self.peek() {
            (self.checkpoint)().map_err(ReadError::Interrupted)?;
            if matches!(token.kind, Kind::Newline | Kind::Semicolon) {
                self.position += 1;
                continue;
            }
            if token.kind == Kind::RightBrace && open.is_some() {
                self.position += 1;
                return Ok(Scope {
                    children,
                    end: token.span.end_exclusive,
                });
            }
            if matches!(token.kind, Kind::RightBrace | Kind::RightParenthesis) {
                return Err(Self::unexpected(token).into());
            }
            self.position += 1;
            let child = self.node(token, depth)?;
            children
                .try_reserve(1)
                .map_err(|_| SyntaxError::new(SyntaxErrorKind::Allocation, child.span))?;
            children.push(child);
        }
        if let Some(open) = open {
            return Err(SyntaxError::new(
                SyntaxErrorKind::Unterminated(Construct::Brace),
                Span::new(open.span.start, self.source.len()),
            )
            .into());
        }
        Ok(Scope {
            children,
            end: self.source.len(),
        })
    }

    fn node(&mut self, first: Token, depth: usize) -> Result<Declaration, ReadError<E>> {
        if self.declarations >= self.limits.declarations {
            return Err(SyntaxError::limit(
                Budget::Declarations,
                self.limits.declarations,
                first.span,
            )
            .into());
        }
        self.declarations += 1;
        let node = Declaration {
            span: first.span,
            name: (first.kind == Kind::Atom).then_some(first.span),
            form: Form::Unknown,
            value: None,
            literal: None,
            children: Vec::new(),
        };
        if first.kind == Kind::LeftBrace {
            return self.block(node, first, depth);
        }
        if first.kind == Kind::LeftParenthesis {
            return self.parentheses(node, first, depth);
        }
        if first.kind != Kind::Atom {
            return Ok(node);
        }
        self.named(node, depth)
    }

    fn named(&mut self, node: Declaration, depth: usize) -> Result<Declaration, ReadError<E>> {
        match self.after_name() {
            Some(
                open @ Token {
                    kind: Kind::LeftBrace,
                    ..
                },
            ) => {
                self.position += 1;
                self.block(node, open, depth)
            }
            Some(
                open @ Token {
                    kind: Kind::LeftParenthesis,
                    ..
                },
            ) => {
                self.position += 1;
                self.parentheses(node, open, depth)
            }
            Some(
                equals @ Token {
                    kind: Kind::Equals, ..
                },
            ) => {
                self.position += 1;
                self.equals(node, equals, depth)
            }
            _ => Ok(Declaration {
                form: Form::Marker,
                ..node
            }),
        }
    }

    fn after_name(&mut self) -> Option<Token> {
        let mut next = self.position;
        while self
            .tokens
            .get(next)
            .is_some_and(|token| token.kind == Kind::Newline)
        {
            next += 1;
        }
        if self
            .tokens
            .get(next)
            .is_some_and(|token| token.kind == Kind::LeftBrace)
        {
            self.position = next;
        }
        self.peek()
    }

    fn block(
        &mut self,
        mut node: Declaration,
        open: Token,
        depth: usize,
    ) -> Result<Declaration, ReadError<E>> {
        self.check_depth(depth + 1, open.span)?;
        if node
            .name
            .and_then(|span| span.text(self.source))
            .is_some_and(|name| name.eq_ignore_ascii_case("Property"))
        {
            if self.properties >= self.limits.properties {
                return Err(SyntaxError::limit(
                    Budget::Properties,
                    self.limits.properties,
                    node.span,
                )
                .into());
            }
            self.properties += 1;
        }
        let scope = self.scope(depth + 1, Some(open))?;
        node.form = Form::Block;
        node.children = scope.children;
        node.span.end_exclusive = scope.end;
        Ok(node)
    }

    fn parentheses(
        &mut self,
        mut node: Declaration,
        open: Token,
        depth: usize,
    ) -> Result<Declaration, ReadError<E>> {
        let start = self.position;
        let close = self.group(open, depth + 1)?;
        let value = Span::new(open.span.end_exclusive, close.span.start);
        self.check_value(value)?;
        node.form = if node.name.is_some() {
            Form::Parentheses
        } else {
            Form::Unknown
        };
        node.value = Some(value);
        node.literal = self.literal(start, self.position - 1);
        node.span.end_exclusive = close.span.end_exclusive;
        Ok(node)
    }

    fn equals(
        &mut self,
        mut node: Declaration,
        equals: Token,
        depth: usize,
    ) -> Result<Declaration, ReadError<E>> {
        let start = self.position;
        while let Some(token) = self.peek() {
            (self.checkpoint)().map_err(ReadError::Interrupted)?;
            if matches!(
                token.kind,
                Kind::Newline | Kind::Semicolon | Kind::RightBrace
            ) {
                break;
            }
            self.position += 1;
            if matches!(token.kind, Kind::LeftBrace | Kind::LeftParenthesis) {
                self.group(token, depth + 1)?;
            } else if token.kind == Kind::RightParenthesis {
                return Err(Self::unexpected(token).into());
            }
        }
        let end = self
            .peek()
            .map_or(self.source.len(), |token| token.span.start);
        let value = Span::new(equals.span.end_exclusive, end);
        self.check_value(value)?;
        node.form = Form::Equals;
        node.value = Some(value);
        node.literal = self.literal(start, self.position);
        node.span.end_exclusive = end;
        Ok(node)
    }

    fn group(&mut self, open: Token, depth: usize) -> Result<Token, ReadError<E>> {
        self.check_depth(depth, open.span)?;
        let (closing, construct) = if open.kind == Kind::LeftBrace {
            (Kind::RightBrace, Construct::Brace)
        } else {
            (Kind::RightParenthesis, Construct::Parenthesis)
        };
        while let Some(token) = self.take() {
            (self.checkpoint)().map_err(ReadError::Interrupted)?;
            if token.kind == closing {
                return Ok(token);
            }
            if matches!(token.kind, Kind::LeftBrace | Kind::LeftParenthesis) {
                self.group(token, depth + 1)?;
            } else if matches!(token.kind, Kind::RightBrace | Kind::RightParenthesis) {
                return Err(Self::unexpected(token).into());
            }
        }
        Err(SyntaxError::new(
            SyntaxErrorKind::Unterminated(construct),
            Span::new(open.span.start, self.source.len()),
        )
        .into())
    }

    const fn check_depth(&self, depth: usize, span: Span) -> Result<(), SyntaxError> {
        if depth > self.limits.depth {
            return Err(SyntaxError::limit(Budget::Depth, self.limits.depth, span));
        }
        Ok(())
    }

    const fn check_value(&self, span: Span) -> Result<(), SyntaxError> {
        if span.end_exclusive - span.start > self.limits.value_bytes {
            return Err(SyntaxError::limit(
                Budget::ValueBytes,
                self.limits.value_bytes,
                span,
            ));
        }
        Ok(())
    }

    fn literal(&self, start: usize, end: usize) -> Option<Span> {
        let tokens = self.tokens.get(start..end)?;
        let mut significant = tokens.iter().filter(|token| token.kind != Kind::Newline);
        let first = significant.next()?;
        (matches!(first.kind, Kind::Atom | Kind::Quoted) && significant.next().is_none())
            .then_some(first.span)
    }

    fn peek(&self) -> Option<Token> {
        self.tokens.get(self.position).copied()
    }

    fn take(&mut self) -> Option<Token> {
        let token = self.peek()?;
        self.position += 1;
        Some(token)
    }

    fn unexpected(token: Token) -> SyntaxError {
        let construct = if token.kind == Kind::RightBrace {
            Construct::Brace
        } else {
            Construct::Parenthesis
        };
        SyntaxError::new(SyntaxErrorKind::UnexpectedClosing(construct), token.span)
    }
}
