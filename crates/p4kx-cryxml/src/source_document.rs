// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::str::Utf8Error;

use roxmltree::{Document as TextDocument, Error as TextParseError, ParsingOptions};
use thiserror::Error;

use crate::depth::{self, TextTooDeep};
use crate::document::Document;
use crate::error::CryXmlError;
use crate::format::Format;
use crate::read::{self, Xml};
use crate::source_element::SourceElement;

/// Admission policy for the text parser.
/// Defaults use the parser's native node index width.
#[derive(Debug, Clone, Copy)]
pub struct SourceReadOptions {
    text_nodes: u32,
    open_text_depth: Option<usize>,
}

impl Default for SourceReadOptions {
    fn default() -> Self {
        Self {
            text_nodes: u32::MAX,
            open_text_depth: None,
        }
    }
}

impl SourceReadOptions {
    /// Sets the parser's maximum text node count.
    #[must_use]
    pub const fn with_text_node_limit(mut self, limit: u32) -> Self {
        self.text_nodes = limit;
        self
    }

    /// Applies an optional preflight bound on simultaneously open text elements.
    #[must_use]
    pub const fn with_open_text_depth_limit(mut self, limit: usize) -> Self {
        self.open_text_depth = Some(limit);
        self
    }
}

/// An XML source that cannot be parsed with the selected policy.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SourceReadError {
    /// The format was unrecognized or the binary document could not be decoded.
    #[error(transparent)]
    CryXml(#[from] CryXmlError),
    /// The text document was not valid UTF-8.
    #[error(transparent)]
    Utf8(#[from] Utf8Error),
    /// The configured open-element depth was exceeded.
    #[error(transparent)]
    TextDepth(#[from] TextTooDeep),
    /// The text parser rejected the source or exceeded its node bound.
    #[error(transparent)]
    Text(#[from] TextParseError),
}

/// A parsed text XML or binary CryXml source with original coordinates.
#[derive(Debug)]
pub struct SourceDocument<'input> {
    document: ParsedDocument<'input>,
}

#[derive(Debug)]
enum ParsedDocument<'input> {
    Text {
        document: TextDocument<'input>,
        prefix_bytes: usize,
    },
    CryXml(Document<'input>),
}

impl<'input> SourceDocument<'input> {
    /// Parses admitted bytes using the existing binary decoder or the text parser.
    ///
    /// # Errors
    ///
    /// Returns the format, UTF-8, policy or parse error for invalid input.
    pub fn parse(bytes: &'input [u8], options: SourceReadOptions) -> Result<Self, SourceReadError> {
        let document = match read::read(bytes)? {
            Xml::Text(bytes) => parse_text(bytes, options)?,
            Xml::CryXml(document) => ParsedDocument::CryXml(document),
        };
        Ok(Self { document })
    }

    /// Returns the original source format.
    #[must_use]
    pub const fn format(&self) -> Format {
        match &self.document {
            ParsedDocument::Text { .. } => Format::Text,
            ParsedDocument::CryXml(document) => Format::CryXml(document.signature()),
        }
    }

    /// Returns whether the binary decoder repaired any original string-table bytes.
    #[must_use]
    pub const fn is_lossy(&self) -> bool {
        match &self.document {
            ParsedDocument::Text { .. } => false,
            ParsedDocument::CryXml(document) => document.is_lossy(),
        }
    }

    /// Borrows the root element of this source.
    #[must_use]
    pub fn root(&self) -> SourceElement<'_> {
        match &self.document {
            ParsedDocument::Text {
                document,
                prefix_bytes,
            } => SourceElement::text(document.root_element(), *prefix_bytes),
            ParsedDocument::CryXml(document) => SourceElement::cryxml(document.root()),
        }
    }
}

fn parse_text(
    bytes: &[u8],
    options: SourceReadOptions,
) -> Result<ParsedDocument<'_>, SourceReadError> {
    let input = str::from_utf8(bytes)?;
    let text = input.trim_start_matches('\u{feff}');
    if let Some(limit) = options.open_text_depth {
        depth::check_text_depth_with_limit(text, limit)?;
    }
    let document = TextDocument::parse_with_options(
        text,
        ParsingOptions {
            nodes_limit: options.text_nodes,
            ..ParsingOptions::default()
        },
    )?;
    Ok(ParsedDocument::Text {
        document,
        prefix_bytes: input.len() - text.len(),
    })
}
