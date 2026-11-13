// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use crate::Reference;
use crate::audio_control_payload::{
    AtlBinaryText, AtlContent, AtlDeclaration, AtlElement, AtlFormat,
};

pub(super) const DOCUMENT: &str = "animation-definition-document";
pub(super) const ELEMENT: &str = "animation-definition-element";
pub(super) const DECLARATION: &str = "animation-definition-declaration";
pub(super) const CONTENT: &str = "animation-definition-content";
pub(super) const BINARY_TEXT: &str = "animation-definition-binary-text";
pub(super) const LINK: &str = "animation-definition-link";
pub(super) const NAMED_CAF: &str = "animation-definition-named-caf";

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub(super) enum DefinitionFamily {
    Chrparams,
    Animevents,
    Adb,
    Bspace,
    Comb,
}

impl DefinitionFamily {
    pub(super) fn of(path: &str) -> Option<Self> {
        let (_, extension) = path.rsplit_once('.')?;
        if extension.eq_ignore_ascii_case("chrparams") {
            Some(Self::Chrparams)
        } else if extension.eq_ignore_ascii_case("animevents") {
            Some(Self::Animevents)
        } else if extension.eq_ignore_ascii_case("adb") {
            Some(Self::Adb)
        } else if extension.eq_ignore_ascii_case("bspace") {
            Some(Self::Bspace)
        } else if extension.eq_ignore_ascii_case("comb") {
            Some(Self::Comb)
        } else {
            None
        }
    }
}

#[derive(Serialize, JsonSchema)]
#[serde(
    tag = "record_type",
    content = "payload",
    rename_all = "kebab-case",
    deny_unknown_fields
)]
pub(super) enum DefinitionPayload<'source> {
    Document(DefinitionDocument),
    Element(AtlElement<'source>),
    Declaration(AtlDeclaration<'source>),
    Content(AtlContent<'source>),
    BinaryText(AtlBinaryText<'source>),
    Link(DefinitionLink<'source>),
    NamedCaf(NamedCaf<'source>),
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DefinitionDocument {
    pub(super) family: DefinitionFamily,
    pub(super) format: Option<AtlFormat>,
    pub(super) nodes: Option<DecimalU64>,
    pub(super) projection: DocumentProjection,
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum DocumentProjection {
    Readable,
    RepairedStrings,
    InvalidSource,
    InvalidSourceSpan,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct DefinitionLink<'source> {
    pub(super) kind: LinkKind,
    pub(super) authored_path: &'source str,
    pub(super) archive_path: Option<String>,
    #[schemars(schema_with = "crate::schema::reference_definition")]
    pub(super) reference: Reference,
}

#[derive(Clone, Copy, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub(super) enum LinkKind {
    TracksDatabase,
    EventDatabase,
    SubDatabase,
    CombinedSpace,
}

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(super) struct NamedCaf<'source> {
    pub(super) name: &'source str,
    pub(super) authored_path: &'source str,
    pub(super) filepath_scope: Option<&'source str>,
    pub(super) archive_path: Option<String>,
    #[schemars(schema_with = "crate::schema::reference_definition")]
    pub(super) reference: Reference,
}
