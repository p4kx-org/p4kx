// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::DecimalU64;
use schemars::JsonSchema;
use serde::Serialize;

use super::blend_reference_payload::{BlendCandidate, BlendReference};
use super::graph_source::{GraphAttribute, GraphNode};
use super::named_dba::Binding;
use super::sub_database_cycles::{SubDatabaseCycle, SubDatabaseEdge};
use super::sub_database_frontier::Frontier;
use super::timeline_payload::{Event, EventAttribute, EventTimeline};
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
pub(super) const NAMED_DBA_BINDING: &str = "animation-definition-named-dba-binding";
pub(super) const EVENT_TIMELINE: &str = "animation-definition-event-timeline";
pub(super) const EVENT: &str = "animation-definition-event";
pub(super) const EVENT_ATTRIBUTE: &str = "animation-definition-event-attribute";
pub(super) const GRAPH_NODE: &str = "animation-definition-graph-node";
pub(super) const SUB_DATABASE_EDGE: &str = "animation-definition-sub-database-edge";
pub(super) const SUB_DATABASE_CYCLE: &str = "animation-definition-sub-database-cycle";
pub(super) const SUB_DATABASE_ATTRIBUTE_FRONTIER: &str =
    "animation-definition-sub-database-attribute-frontier";
pub(super) const SUB_DATABASE_TARGET_FRONTIER: &str =
    "animation-definition-sub-database-target-frontier";
pub(super) const BLEND_REFERENCE: &str = "animation-definition-blend-space-reference";
pub(super) const BLEND_CANDIDATE: &str = "animation-definition-blend-space-candidate";
pub(super) const GRAPH_ATTRIBUTE: &str = "animation-definition-graph-attribute";

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
    NamedDbaBinding(Binding<'source>),
    EventTimeline(EventTimeline<'source>),
    Event(Event),
    EventAttribute(EventAttribute<'source>),
    GraphNode(GraphNode<'source>),
    GraphAttribute(GraphAttribute<'source>),
    SubDatabaseEdge(SubDatabaseEdge),
    SubDatabaseCycle(SubDatabaseCycle),
    SubDatabaseFrontier(Frontier),
    BlendSpaceReference(BlendReference<'source>),
    BlendSpaceCandidate(BlendCandidate),
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
    TagDefinition,
    FragmentDefinition,
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
