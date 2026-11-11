// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Reads `CryXml`, the binary XML encoding `CryEngine` uses for many Entries, and writes it as
//! UTF-8 XML.
//!
//! [`detect`] recognises the `CryXmlB`, `CryXml` and `CRY3SDK` signatures and text XML; [`parse`]
//! turns a `CryXml` byte slice of either byte order into a [`Document`] that borrows its strings
//! from the input; [`read`] does both and passes text XML through unchanged.

mod comparison;
mod depth;
mod document;
mod element;
mod error;
mod escape;
mod format;
mod header;
mod names;
mod read;
mod source_attribute;
mod source_content;
mod source_declaration;
mod source_document;
mod source_element;
mod source_position;
mod source_start_tag;
mod source_syntax;
mod strings;
mod write;

pub use comparison::{TreeComparison, compare_trees};
pub use depth::{MAX_TEXT_DEPTH, TextTooDeep, check_text_depth};
pub use document::{Document, parse};
pub use element::{Attribute, Attributes, Children, Element};
pub use error::{CryXmlError, Table};
pub use format::{ByteOrder, Format, Signature, detect};
pub use read::{Xml, read};
pub use source_attribute::SourceAttribute;
pub use source_content::{SourceContent, SourceContentPosition, SourceContents, SourceText};
pub use source_declaration::{SourceDeclaration, SourceDeclarationKind};
pub use source_document::{SourceDocument, SourceReadError, SourceReadOptions};
pub use source_element::SourceElement;
pub use source_position::{SourceAttributePosition, SourceElementPosition};
pub use source_syntax::{SourceSpanError, SourceSyntax};
