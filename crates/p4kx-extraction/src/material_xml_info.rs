// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::borrow::Cow;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Bounded material XML syntax, independent of original byte retention and surface interpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialElementInfo<'source> {
    /// Existing XML-reader element name.
    pub name: Cow<'source, str>,
    /// Existing decoded attributes in reader order.
    pub attributes: Vec<MaterialParameterInfo<'source>>,
    /// Retained children in source order; their original ordinals remain in syntax.
    pub children: Vec<Self>,
    /// Encoding-aware authored names, namespace declarations and content.
    pub syntax: MaterialElementSyntaxInfo<'source>,
}

/// One decoded material attribute without numeric reinterpretation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[non_exhaustive]
pub struct MaterialParameterInfo<'source> {
    /// Reader-exposed name with its original case.
    pub name: Cow<'source, str>,
    /// Uninterpreted decoded value.
    pub value: Cow<'source, str>,
}

/// Authored syntax supplementing the existing material element view.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
#[non_exhaustive]
pub struct MaterialElementSyntaxInfo<'source> {
    /// Original element-child position, absent for the captured root.
    pub ordinal: Option<usize>,
    /// Original qualified name, including its prefix.
    pub qualified_name: Cow<'source, str>,
    /// Expanded URI proved for plain XML; binary namespace semantics stay unasserted.
    pub namespace: Option<Cow<'source, str>>,
    /// Attributes and namespace declarations in one authored order.
    pub attributes: Vec<MaterialAttributeInfo<'source>>,
    /// Plain XML mixed-content sequence; no original-byte offset is implied.
    pub content: Vec<MaterialContentInfo<'source>>,
    /// Binary node text, with unspecified ordering relative to its children.
    pub binary_text: Option<Cow<'source, str>>,
}

/// One original attribute and its independent semantic/lexical values.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[schemars(transform = crate::projection_schema::require_properties)]
#[non_exhaustive]
pub struct MaterialAttributeInfo<'source> {
    /// Original qualified attribute name.
    pub name: Cow<'source, str>,
    /// XML-decoded value without float or flag conversion.
    pub value: Cow<'source, str>,
    /// Original UTF-8 XML value token without quotes; absent for binary XML.
    pub lexeme: Option<Cow<'source, str>>,
    /// Attribute or namespace-declaration meaning.
    pub kind: MaterialAttributeKindInfo<'source>,
}

/// Explicit namespace interpretation of an authored attribute.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialAttributeKindInfo<'source> {
    /// A regular attribute.
    #[schemars(transform = crate::projection_schema::require_properties)]
    Attribute {
        /// Expanded URI when proved; library encoding distinguishes unqualified plain input from unresolved binary semantics.
        namespace: Option<Cow<'source, str>>,
    },
    /// A namespace declaration.
    #[schemars(transform = crate::projection_schema::require_properties)]
    Namespace {
        /// Authored prefix, absent for the default namespace.
        prefix: Option<Cow<'source, str>>,
    },
    /// The reader supplies a kind this projection does not support.
    Unsupported,
}

/// An ordered plain-XML content unit or an original element-child ordinal.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialContentInfo<'source> {
    /// Plain XML text with a separate original token.
    Text {
        /// XML-decoded value.
        value: Cow<'source, str>,
        /// Original UTF-8 source token, including any CDATA/entity spelling.
        lexeme: Cow<'source, str>,
    },
    /// An original element-child position, including a diagnosed unretained child.
    Element {
        /// Original element-child ordinal.
        ordinal: usize,
    },
    /// An authored comment.
    Comment {
        /// Content without comment delimiters.
        value: Cow<'source, str>,
    },
    /// An authored processing instruction.
    #[schemars(transform = crate::projection_schema::require_properties)]
    ProcessingInstruction {
        /// Instruction target.
        target: Cow<'source, str>,
        /// Optional instruction data.
        value: Option<Cow<'source, str>>,
    },
    /// The reader supplies content this projection does not support.
    Unsupported,
}
