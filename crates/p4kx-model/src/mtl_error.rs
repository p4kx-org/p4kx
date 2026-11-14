// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::{CryXmlError, TextTooDeep};
use thiserror::Error;

/// A failure to read a `.mtl` file.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum MaterialError {
    /// Plain XML input contains invalid UTF-8 bytes.
    #[error("the material XML source is not UTF-8")]
    TextEncoding(#[from] std::str::Utf8Error),
    /// A validated XML source span cannot be retained consistently.
    #[error("the material XML source span is inconsistent")]
    SourceSpan,
    /// The caller supplied zero budgets or an unsupported recursion bound.
    #[error("invalid material source projection limits")]
    InvalidSourceLimits,
    /// The structured input exceeds the configured source byte bound.
    #[error("material source exceeds {limit} bytes")]
    SourceBytes {
        /// Actual input bound applied before parsing.
        limit: usize,
    },
    /// The file is no readable `CryXml`.
    #[error("the file is not valid CryXml")]
    CryXml(#[from] CryXmlError),
    /// The file is no well-formed text XML.
    #[error("the file is not well-formed XML")]
    Xml(#[from] roxmltree::Error),
    /// The text XML nests its elements too deeply to be read.
    #[error("the file is not readable XML: {0}")]
    TooDeep(#[from] TextTooDeep),
    /// The root element is no `Material`.
    #[error("the root element is {found}, not Material")]
    NotMaterial {
        /// The root element's name.
        found: String,
    },
    /// The file holds more of something than a material library may.
    #[error("the file holds more than {limit} {what}")]
    TooMany {
        /// What there is too much of.
        what: &'static str,
        /// The most allowed.
        limit: usize,
    },
}
