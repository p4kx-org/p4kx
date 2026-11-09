// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::convert::Infallible;
use std::str;

use crate::{Budget, Definition, Encoding, Limits, ReadError, Span, SyntaxError, SyntaxErrorKind};
use crate::{lexer, parser};

/// Reads ordered declarations while borrowing the exact original source.
///
/// # Errors
/// Returns unsupported encoding, malformed structure, allocation or resource-limit failures.
pub fn read(source: &[u8], limits: Limits) -> Result<Definition<'_>, SyntaxError> {
    read_with(source, limits, || Ok::<(), Infallible>(())).map_err(|error| match error {
        ReadError::Syntax(error) => error,
        ReadError::Interrupted(never) => match never {},
    })
}

/// Reads declarations with fallible checkpoints during lexical and structural work.
/// The checkpoint's original error remains available without string conversion.
///
/// # Errors
/// Returns structural/resource failures or the unchanged checkpoint error.
pub fn read_with<E>(
    source: &[u8],
    limits: Limits,
    mut checkpoint: impl FnMut() -> Result<(), E>,
) -> Result<Definition<'_>, ReadError<E>> {
    checkpoint().map_err(ReadError::Interrupted)?;
    if source.len() > limits.source_bytes {
        return Err(SyntaxError::limit(
            Budget::SourceBytes,
            limits.source_bytes,
            Span::new(0, source.len()),
        )
        .into());
    }
    limits.validate_configuration()?;
    let text = str::from_utf8(source).map_err(|error| {
        let start = error.valid_up_to();
        let end = error.error_len().map_or(source.len(), |size| start + size);
        SyntaxError::new(SyntaxErrorKind::UnknownEncoding, Span::new(start, end))
    })?;
    checkpoint().map_err(ReadError::Interrupted)?;
    let (encoding, start) = if source.starts_with(b"\xef\xbb\xbf") {
        (Encoding::Utf8Bom, 3)
    } else {
        (Encoding::Utf8, 0)
    };
    let tokens = lexer::lex(source, start, limits, &mut checkpoint)?;
    let declarations = parser::parse(text, &tokens, limits, &mut checkpoint)?;
    checkpoint().map_err(ReadError::Interrupted)?;
    Ok(Definition::new(text, encoding, declarations))
}
