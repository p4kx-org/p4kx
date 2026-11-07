// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Bounded source-preserving reads of shader extension declarations.

mod declaration;
mod definition;
mod error;
mod field;
mod lexer;
mod limits;
mod mask;
mod parser;
mod read;
mod span;

pub use declaration::{Declaration, Form};
pub use definition::{Definition, Encoding, Property};
pub use error::{Budget, Construct, ReadError, SyntaxError, SyntaxErrorKind};
pub use field::{Field, PropertyFieldKind, RootFieldKind};
pub use limits::Limits;
pub use mask::{Mask, MaskIssue};
pub use read::{read, read_with};
pub use span::Span;
