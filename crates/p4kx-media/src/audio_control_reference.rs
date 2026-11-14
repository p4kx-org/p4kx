// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::sound;
use p4kx_extraction::{DecimalU64, SourceUseRole};
use schemars::JsonSchema;
use serde::Serialize;

use crate::audio_control_fields::{AtlFamily, AtlField};
use crate::audio_control_graph::{NameScope, Named, NamedKind, Site};
use crate::audio_control_origin as origins;
use crate::audio_control_payload::{self as wire, AtlPayload};
use crate::audio_foreign::Selector;
use crate::origin_input::MediaOriginInput;

#[path = "audio_control_path.rs"]
pub(crate) mod paths;
use crate::{DirectoryLookup, MediaError, MediaKind, MediaWriter, Status, Subresource};

#[derive(Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub(crate) struct AtlReference<'document> {
    target: AtlReferenceTarget<'document>,
    link: AtlReferenceLink,
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "type", rename_all = "kebab-case", deny_unknown_fields)]
enum AtlReferenceTarget<'document> {
    WwiseEvent {
        name: &'document str,
        short_id: Option<u32>,
    },
    WwiseObject {
        kind: AtlFamily,
        name: &'document str,
        short_id: Option<u32>,
    },
    Named {
        kind: NamedKind,
        name: &'document str,
    },
    File {
        kind: paths::AtlFileKind,
        path: &'document str,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(tag = "status", rename_all = "kebab-case", deny_unknown_fields)]
enum AtlReferenceLink {
    SoundbankSearch {
        query_index: DecimalU64,
    },
    NotSelected,
    ScopedSearch {
        query_index: DecimalU64,
    },
    PathSearch {
        resolution: paths::PathDisposition,
        observed_candidates: DecimalU64,
        candidates_complete: bool,
    },
    Unavailable {
        reason: AtlReferenceUnavailable,
    },
}

#[derive(Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
enum AtlReferenceUnavailable {
    EmptyName,
    NoOwningPreload,
    UnsafePath,
    UnprovedTargetPath,
    TargetNotIndexed,
}

fn wwise(
    directory: &DirectoryLookup<'_>,
    input: &MediaOriginInput,
    declaration: NameDeclaration<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    writer.start_reference()?;
    let evidence = origins::capture(
        input,
        declaration.site.position,
        "declaration",
        SourceUseRole::Conversion,
        writer,
    )?;
    let prepared = origins::prepared(input, evidence.clone(), writer)?;
    let is_event = declaration.family == AtlFamily::WwiseEvent;
    let target = if is_event {
        AtlReferenceTarget::WwiseEvent {
            name: declaration.name,
            short_id: declaration.short_id,
        }
    } else {
        AtlReferenceTarget::WwiseObject {
            kind: declaration.family,
            name: declaration.name,
            short_id: (!declaration.name.is_empty()).then(|| sound::wwise_id(declaration.name)),
        }
    };
    let link = match (
        is_event,
        declaration.short_id,
        writer.has_kind(MediaKind::Audio),
    ) {
        _ if declaration.name.is_empty() => AtlReferenceLink::Unavailable {
            reason: AtlReferenceUnavailable::EmptyName,
        },
        (true, Some(id), true) => {
            let query_index =
                directory
                    .index
                    .foreign_banks
                    .request(id, Selector::Event, evidence, writer)?;
            AtlReferenceLink::SoundbankSearch { query_index }
        }
        (_, _, false) => AtlReferenceLink::NotSelected,
        _ => AtlReferenceLink::Unavailable {
            reason: AtlReferenceUnavailable::TargetNotIndexed,
        },
    };
    prepared.write(
        writer,
        &Subresource::new(wire::REFERENCE, declaration.site.indices.to_vec()),
        Status::Complete,
        &AtlPayload::Reference(AtlReference { target, link }),
        &[],
    )
}

#[derive(Clone, Copy)]
pub(crate) struct NameDeclaration<'document> {
    pub(crate) family: AtlFamily,
    pub(crate) field: AtlField,
    pub(crate) name: &'document str,
    pub(crate) short_id: Option<u32>,
    pub(crate) site: Site,
    pub(crate) preload: Option<u64>,
}

pub(crate) fn authored_name(
    directory: &DirectoryLookup<'_>,
    input: &MediaOriginInput,
    declaration: NameDeclaration<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    if matches!(declaration.field, AtlField::ExternalFilename) {
        return paths::project(
            directory,
            input,
            declaration,
            paths::AtlFileKind::ExternalMedia,
            writer,
        );
    }
    if matches!(declaration.field, AtlField::WwiseName) {
        return match declaration.family {
            AtlFamily::WwiseFile => paths::project(
                directory,
                input,
                declaration,
                paths::AtlFileKind::Bank,
                writer,
            ),
            AtlFamily::WwiseExternalSource => paths::project(
                directory,
                input,
                declaration,
                paths::AtlFileKind::ExternalMedia,
                writer,
            ),
            _ => wwise(directory, input, declaration, writer),
        };
    }
    if declaration.name.is_empty() {
        return named_edge(directory, input, declaration, writer);
    }
    let kind = match (declaration.family, declaration.field) {
        (AtlFamily::PreloadRequest, AtlField::Name) => Some(NamedKind::PreloadRequest),
        (AtlFamily::Rtpc, AtlField::Name) => Some(NamedKind::Rtpc),
        (AtlFamily::ConfigGroup, AtlField::Name) => Some(NamedKind::ConfigGroup),
        _ => None,
    };
    if let Some(kind) = kind
        && let Some(scope) = name_scope(kind, declaration)
    {
        directory.index.atl_controls.target(
            Named {
                kind,
                scope,
                name: declaration.name,
            },
            declaration.site,
            input,
        )?;
    }
    named_edge(directory, input, declaration, writer)
}

fn named_edge(
    directory: &DirectoryLookup<'_>,
    input: &MediaOriginInput,
    declaration: NameDeclaration<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let kind = match (declaration.family, declaration.field) {
        (AtlFamily::Preload, AtlField::Name) => NamedKind::PreloadRequest,
        (AtlFamily::RtpcDependee, AtlField::Name) => NamedKind::Rtpc,
        (AtlFamily::Platform, AtlField::ConfigGroupName) => NamedKind::ConfigGroup,
        _ => return Ok(()),
    };
    writer.start_reference()?;
    let evidence = origins::capture(
        input,
        declaration.site.position,
        "declaration",
        SourceUseRole::Selector,
        writer,
    )?;
    let prepared = origins::prepared(input, evidence.clone(), writer)?;
    let link = match name_scope(kind, declaration) {
        _ if declaration.name.is_empty() => AtlReferenceLink::Unavailable {
            reason: AtlReferenceUnavailable::EmptyName,
        },
        None => AtlReferenceLink::Unavailable {
            reason: AtlReferenceUnavailable::NoOwningPreload,
        },
        Some(scope) => {
            let query_index = directory.index.atl_controls.request(
                Named {
                    kind,
                    scope,
                    name: declaration.name,
                },
                declaration.site,
                input,
                evidence,
                writer,
            )?;
            AtlReferenceLink::ScopedSearch { query_index }
        }
    };
    prepared.write(
        writer,
        &Subresource::new(wire::REFERENCE, declaration.site.indices.to_vec()),
        Status::Complete,
        &AtlPayload::Reference(AtlReference {
            target: AtlReferenceTarget::Named {
                kind,
                name: declaration.name,
            },
            link,
        }),
        &[],
    )
}

const fn name_scope(kind: NamedKind, declaration: NameDeclaration<'_>) -> Option<NameScope> {
    match kind {
        NamedKind::ConfigGroup => match declaration.preload {
            Some(element) => Some(NameScope::Preload {
                entry: declaration.site.entry,
                element,
            }),
            None => None,
        },
        NamedKind::PreloadRequest | NamedKind::Rtpc => Some(NameScope::Directory),
    }
}
