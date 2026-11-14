// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::RawElement;

/// The source encoding of an authored material library.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialEncoding {
    /// Plain XML whose lexical spans address the original UTF-8 bytes.
    Text,
    /// Binary XML whose node content has no mixed-content interleaving declaration.
    CryXml,
}

/// Bounds for one structured material projection, independent of exact byte retention.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MaterialSourceLimits {
    /// Maximum input size accepted by the structured parser.
    pub source_bytes: usize,
    /// Maximum retained authored elements, or `None` for no element-count bound.
    pub elements: Option<usize>,
    /// Maximum owned text and syntax storage in the canonical authored tree.
    pub text_bytes: usize,
    /// Maximum authored element depth, with the root at zero.
    pub depth: usize,
}

impl Default for MaterialSourceLimits {
    fn default() -> Self {
        Self {
            source_bytes: 16 << 20,
            elements: None,
            text_bytes: 4 << 20,
            depth: 32,
        }
    }
}

/// Why an authored material projection cannot claim complete structured coverage.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialSourceLossKind {
    /// The binary XML reader repaired invalid UTF-8 string-table bytes.
    RepairedEncoding,
    /// Additional authored elements exceed the configured bound.
    Elements,
    /// Additional syntax/text storage exceeds the configured bound.
    TextBytes,
    /// A populated child list extends beyond the configured depth.
    Depth,
}

/// One bounded projection-loss category and its first observed source location.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MaterialSourceLoss {
    /// Actual boundary reached by the reader.
    pub kind: MaterialSourceLossKind,
    /// Original element ordinals from the root, without compacting skipped children.
    pub indices: Vec<usize>,
}

/// The existing surface-reader disposition of one original submaterial position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum MaterialEntryDisposition {
    /// The entry is a material interpreted by the surface reader.
    Material,
    /// The unknown entry keeps its ordinal and uses the engine-default surface.
    EngineDefault,
}

/// An original submaterial slot, including unknown authored entry types.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MaterialSourceEntry {
    /// Original position in the first authored `SubMaterials` list.
    pub ordinal: usize,
    /// Unmodified element name exposed by the source XML reader.
    pub element_name: String,
    /// Existing rendering disposition, separate from raw source retention.
    pub disposition: MaterialEntryDisposition,
}

/// One bounded authored tree, separate from interpreted material surface values.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct MaterialSource {
    /// Input encoding actually observed by the reader.
    pub encoding: MaterialEncoding,
    /// Complete retained root syntax, or absent when its own storage exceeds the bound.
    pub root: Option<RawElement>,
    /// Actual structured-projection boundaries, empty when none was reached.
    pub losses: Vec<MaterialSourceLoss>,
    /// Original position of the selected `SubMaterials` list among the root's element children.
    pub submaterial_list_ordinal: Option<usize>,
    /// Original submaterial positions; empty for a single-material library.
    pub entries: Vec<MaterialSourceEntry>,
}

/// One original attribute, including namespace declarations in their authored order.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct AuthoredAttribute {
    /// Qualified name as authored, including its original prefix and case.
    pub name: String,
    /// Decoded XML value without material-specific numeric interpretation.
    pub value: String,
    /// Original plain-XML value token excluding quotes; absent for binary XML.
    pub lexeme: Option<String>,
    /// Namespace interpretation proved by the XML reader.
    pub kind: AuthoredAttributeKind,
}

/// Namespace meaning of one authored attribute.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthoredAttributeKind {
    /// A regular attribute with its expanded namespace, when it has one.
    Attribute {
        /// Expanded URI when proved; binary namespace semantics are not inferred.
        namespace: Option<String>,
    },
    /// A namespace declaration; an absent prefix declares the default namespace.
    Namespace {
        /// Authored prefix, absent for the default namespace declaration.
        prefix: Option<String>,
    },
}

/// Ordered content exposed by the plain XML reader or an original child ordinal.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum AuthoredContent {
    /// Plain XML text with an independent unmodified source token.
    Text {
        /// XML-decoded text.
        value: String,
        /// Unmodified token span from the original plain XML.
        lexeme: String,
    },
    /// An original child ordinal; a budget may leave that child unretained.
    Element {
        /// Original element-child position in the parent.
        ordinal: usize,
    },
    /// A plain XML comment, without its delimiters.
    Comment {
        /// Comment content without delimiters.
        value: String,
    },
    /// A plain XML processing instruction.
    ProcessingInstruction {
        /// Original processing-instruction target.
        target: String,
        /// Optional instruction value.
        value: Option<String>,
    },
}

/// Source syntax supplementing the existing decoded element view.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub struct ElementAuthored {
    /// Original child ordinal in the parent, absent on the captured root.
    pub ordinal: Option<usize>,
    /// Qualified element name as authored.
    pub qualified_name: String,
    /// Expanded element URI proved for plain XML; binary declaration strings stay unresolved.
    pub namespace: Option<String>,
    /// Attributes and namespace declarations in one original ordered list.
    pub attributes: Vec<AuthoredAttribute>,
    /// Plain XML mixed content and child ordinals in source order.
    pub content: Vec<AuthoredContent>,
    /// Binary XML's own node content, whose ordering relative to children is unspecified.
    pub binary_text: Option<String>,
}
