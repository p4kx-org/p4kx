// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod compound;
mod serializer;

use serde::Serialize;

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Bool(bool),
    Signed(i128),
    Unsigned(u128),
    Float32(u32),
    Float64(u64),
    Character(char),
    Text(String),
    Bytes(Vec<u8>),
    None,
    Some,
    Unit,
    Label(&'static str),
    Length(Option<usize>),
    Variant(&'static str, u32, &'static str),
    Start(&'static str),
    End,
}

#[derive(Debug, thiserror::Error)]
#[error("canonical property-set serialization failed: {0}")]
pub(super) struct Error(String);

impl serde::ser::Error for Error {
    fn custom<T: std::fmt::Display>(message: T) -> Self {
        Self(message.to_string())
    }
}

enum Output<'t> {
    Capture(&'t mut Vec<Token>),
    Compare {
        tokens: &'t [Token],
        at: usize,
        equal: bool,
    },
}

struct Emitter<'t>(Output<'t>);

impl Emitter<'_> {
    fn emit(&mut self, token: Token) {
        match &mut self.0 {
            Output::Capture(tokens) => tokens.push(token),
            Output::Compare { tokens, at, equal } => {
                *equal &= tokens.get(*at) == Some(&token);
                *at = at.saturating_add(1);
            }
        }
    }
}

pub(super) fn equal(a: &impl Serialize, b: &impl Serialize) -> Result<bool, serde_json::Error> {
    let compare = || -> Result<bool, Error> {
        let mut tokens = Vec::new();
        a.serialize(&mut Emitter(Output::Capture(&mut tokens)))?;
        let mut output = Emitter(Output::Compare {
            tokens: &tokens,
            at: 0,
            equal: true,
        });
        b.serialize(&mut output)?;
        Ok(matches!(output.0, Output::Compare { at, equal: true, .. } if at == tokens.len()))
    };
    compare().map_err(<serde_json::Error as serde::ser::Error>::custom)
}
