// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::DecimalU64;
use p4kx_extraction::cryxml::{Format, Signature, SourceDeclarationKind};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_control_fields::{AtlFamily, AtlField, AtlScalar};
use crate::audio_control_graph::rows::{AtlReferenceCandidate, AtlReferenceResolution};
use crate::audio_control_reference::AtlReference;
use crate::audio_control_reference::paths::AtlPathCandidate;

pub(crate) const DOCUMENT: &str = "audio-control-document";
pub(crate) const ELEMENT: &str = "audio-control-element";
pub(crate) const DECLARATION: &str = "audio-control-declaration";
pub(crate) const CONTENT: &str = "audio-control-content";
pub(crate) const BINARY_TEXT: &str = "audio-control-binary-text";
pub(crate) const DEFINITION: &str = "audio-control-definition";
pub(crate) const FIELD: &str = "audio-control-field";
pub(crate) const REFERENCE: &str = "audio-control-reference";
pub(crate) const FAILURE: &str = "audio-control-failure";

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(crate) enum AtlPayload<'document> {
    Document(AtlDocument),
    Element(AtlElement<'document>),
    Declaration(AtlDeclaration<'document>),
    Content(AtlContent<'document>),
    BinaryText(AtlBinaryText<'document>),
    Definition(AtlDefinition),
    Field(AtlFieldRecord<'document>),
    Reference(AtlReference<'document>),
    ReferenceResolution(AtlReferenceResolution),
    ReferenceCandidate(AtlReferenceCandidate),
    PathCandidate(AtlPathCandidate),
    Failure(AtlFailure),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlDocument {
    pub(crate) format: AtlFormat,
    pub(crate) root_is_atl_config: bool,
    pub(crate) repaired_string_bytes: bool,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AtlFormat {
    PlainXml,
    CryXml,
    CryXmlB,
    Cry3Sdk,
}

impl From<Format> for AtlFormat {
    fn from(format: Format) -> Self {
        match format {
            Format::Text => Self::PlainXml,
            Format::CryXml(Signature::CryXml) => Self::CryXml,
            Format::CryXml(Signature::CryXmlB) => Self::CryXmlB,
            Format::CryXml(Signature::Cry3Sdk) => Self::Cry3Sdk,
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlElement<'document> {
    pub(crate) qualified_name: &'document str,
    pub(crate) namespace: Option<&'document str>,
    pub(crate) location: AtlLocation,
    pub(crate) position: AtlPosition,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlDeclaration<'document> {
    pub(crate) qualified_name: &'document str,
    pub(crate) value: &'document str,
    pub(crate) lexeme: Option<&'document str>,
    pub(crate) kind: AtlDeclarationKind<'document>,
    pub(crate) position: AtlPosition,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum AtlDeclarationKind<'document> {
    Attribute { namespace: Option<&'document str> },
    Namespace { prefix: Option<&'document str> },
}

impl<'document> TryFrom<SourceDeclarationKind<'document>> for AtlDeclarationKind<'document> {
    type Error = p4kx_extraction::cryxml::SourceSpanError;

    fn try_from(kind: SourceDeclarationKind<'document>) -> Result<Self, Self::Error> {
        Ok(match kind {
            SourceDeclarationKind::Attribute { namespace } => Self::Attribute { namespace },
            SourceDeclarationKind::Namespace { prefix } => Self::Namespace { prefix },
            _ => return Err(p4kx_extraction::cryxml::SourceSpanError),
        })
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlContent<'document> {
    pub(crate) location: AtlLocation,
    pub(crate) position: AtlPosition,
    pub(crate) content: AtlContentValue<'document>,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum AtlContentValue<'document> {
    Text {
        value: &'document str,
        lexeme: &'document str,
    },
    Comment {
        value: &'document str,
        lexeme: &'document str,
    },
    ProcessingInstruction {
        target: &'document str,
        value: Option<&'document str>,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlBinaryText<'document> {
    pub(crate) position: AtlPosition,
    pub(crate) value: &'document str,
}

#[derive(Clone, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlLocation {
    pub(crate) parent: Option<DecimalU64>,
    pub(crate) sibling_ordinal: DecimalU64,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
pub(crate) enum AtlPosition {
    Text {
        start: DecimalU64,
        end_exclusive: DecimalU64,
    },
    CryXmlNode {
        node_index: u32,
    },
    CryXmlAttribute {
        node_index: u32,
        attribute_index: u32,
    },
    CryXmlContent {
        node_index: u32,
    },
}

impl AtlPosition {
    pub(crate) const fn text(range: &Range<usize>) -> Self {
        Self::Text {
            start: DecimalU64::new(range.start as u64),
            end_exclusive: DecimalU64::new(range.end as u64),
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlDefinition {
    pub(crate) family: AtlFamily,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlFieldRecord<'document> {
    pub(crate) field: AtlField,
    pub(crate) typed_value: AtlScalar<'document>,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum AtlFailure {
    UnsupportedReader,
    BinaryOrUnknownFormat,
    TextEncoding,
    TextXml,
    SourceSpan,
}
