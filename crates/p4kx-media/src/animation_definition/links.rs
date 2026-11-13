// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttributePosition, SourceDeclaration, SourceDeclarationKind, SourceElement,
};
use p4kx_extraction::{FieldPath, SourceUseRole};

use super::origins;
use super::payload::{self as wire, DefinitionFamily, DefinitionLink, DefinitionPayload, LinkKind};
use super::projection::DefinitionProjection;
use crate::audio_control_payload::AtlPosition;
use crate::origin_reference::{ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource};
use crate::xml_source_projection::declaration_position;

pub(super) struct LinkSelector {
    pub(super) kind: LinkKind,
    name_position: Option<SourceAttributePosition>,
}
use crate::{
    CandidateInventory, MediaError, MediaKind, MediaWriter, OriginalIdentifier,
    ReferenceUnavailable, Status, Subresource, origin_error,
};

pub(super) fn kind(
    family: DefinitionFamily,
    element: SourceElement<'_>,
    parent: Option<&str>,
    attribute: &SourceDeclaration<'_>,
) -> Option<LinkSelector> {
    if element.namespace().is_some()
        || !matches!(
            attribute.kind(),
            SourceDeclarationKind::Attribute { namespace: None }
        )
    {
        return None;
    }
    let tag = element.name();
    let name = attribute.qualified_name();
    if matches!(family, DefinitionFamily::Chrparams)
        && tag.eq_ignore_ascii_case("Animation")
        && parent.is_some_and(|name| name.eq_ignore_ascii_case("AnimationList"))
        && name.eq_ignore_ascii_case("path")
    {
        let mut names = element.attributes().filter(|attribute| {
            attribute.namespace().is_none() && attribute.name().eq_ignore_ascii_case("name")
        });
        let first = names.next()?;
        if names.next().is_some() {
            return None;
        }
        if first.value().eq_ignore_ascii_case("$TracksDatabase") {
            return Some(LinkSelector {
                kind: LinkKind::TracksDatabase,
                name_position: Some(first.position().clone()),
            });
        }
        if first.value().eq_ignore_ascii_case("$AnimEventDatabase") {
            return Some(LinkSelector {
                kind: LinkKind::EventDatabase,
                name_position: Some(first.position().clone()),
            });
        }
    }
    if matches!(family, DefinitionFamily::Adb)
        && tag.eq_ignore_ascii_case("SubADB")
        && name.eq_ignore_ascii_case("File")
    {
        return Some(LinkSelector {
            kind: LinkKind::SubDatabase,
            name_position: None,
        });
    }
    if matches!(family, DefinitionFamily::Comb)
        && tag.eq_ignore_ascii_case("BlendSpace")
        && name.eq_ignore_ascii_case("AName")
    {
        return Some(LinkSelector {
            kind: LinkKind::CombinedSpace,
            name_position: None,
        });
    }
    None
}

pub(super) fn write(
    projection: &DefinitionProjection<'_, '_>,
    selector: &LinkSelector,
    authored_path: &str,
    position: AtlPosition,
    indices: [u64; 2],
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let proof = origins::position(projection.input, position, SourceUseRole::Selector, writer)?;
    let selection = selector.proof(projection.input, &proof, writer)?;
    let mut prepared = origins::record(projection.input, proof.clone(), writer)?;
    crate::audio_record_origins::field(
        &mut prepared,
        "/data/payload/kind",
        selection.clone(),
        writer,
    )?;
    crate::audio_record_origins::field(
        &mut prepared,
        "/data/payload/archive_path",
        selection.clone(),
        writer,
    )?;
    let kind = selector.kind;
    let original = OriginalIdentifier::new_with_path(authored_path);
    let archive_path = target_path(kind, authored_path);
    let reference = if let Some(path) = &archive_path {
        let lookup = projection.context.directory.lookup_input_observed(
            original,
            path,
            Some(kind.target_kind()),
            writer,
        )?;
        let origins = ReferenceOriginInputs {
            original: &proof,
            selector: &selection,
            inventory: &lookup.inventory,
            semantic: &proof,
            candidates: &lookup.evidence,
            read_selected: false,
            identifier: ReferenceIdentifier::Path,
            subresource: ReferenceSubresource::None,
        };
        let address = FieldPath::parse("/data/payload/reference")
            .map_err(|cause| origin_error::address(&cause))?;
        origins.write(&mut prepared, &address, &lookup.lookup.reference, writer)?;
        lookup.lookup.reference
    } else {
        writer.resolve_reference(
            original,
            [],
            CandidateInventory::Unavailable(ReferenceUnavailable::InvalidIdentifier),
        )?
    };
    prepared.write(
        writer,
        &Subresource::new(wire::LINK, indices.to_vec()),
        Status::Complete,
        &DefinitionPayload::Link(DefinitionLink {
            kind,
            authored_path,
            archive_path,
            reference,
        }),
        &[],
    )
}

impl LinkSelector {
    fn proof(
        &self,
        input: &crate::origin_input::MediaOriginInput,
        path: &crate::origin_contributions::OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<crate::origin_contributions::OriginContributions, MediaError> {
        let mut selection = writer.capture_origins();
        selection.include(path, writer)?;
        if let Some(position) = &self.name_position {
            let position = declaration_position(position)
                .map_err(|_| MediaError::Contract("invalid XML animation selector span".into()))?;
            selection.include(
                &origins::position(input, position, SourceUseRole::Selector, writer)?,
                writer,
            )?;
        }
        Ok(selection.finish())
    }
}

impl LinkKind {
    const fn target_kind(self) -> MediaKind {
        match self {
            Self::TracksDatabase => MediaKind::Animation,
            Self::EventDatabase | Self::SubDatabase | Self::CombinedSpace => {
                MediaKind::AnimationDefinition
            }
        }
    }
}

fn target_path(kind: LinkKind, path: &str) -> Option<String> {
    let (_, extension) = path.rsplit_once('.')?;
    let supported = match kind {
        LinkKind::TracksDatabase => extension.eq_ignore_ascii_case("dba"),
        LinkKind::EventDatabase => extension.eq_ignore_ascii_case("animevents"),
        LinkKind::SubDatabase => extension.eq_ignore_ascii_case("adb"),
        LinkKind::CombinedSpace => {
            extension.eq_ignore_ascii_case("bspace") || extension.eq_ignore_ascii_case("comb")
        }
    };
    if !supported
        || path.split(['/', '\\']).any(|part| {
            matches!(part, "" | "." | "..")
                || part.contains([':', '\0', '!', '*', '?'])
                || part.ends_with([' ', '.'])
        })
    {
        return None;
    }
    let mut parts = path.split(['/', '\\']);
    let first = parts.next()?;
    if first.eq_ignore_ascii_case("data") && parts.next().is_some() {
        Some(path.replace('\\', "/"))
    } else if first.eq_ignore_ascii_case("animations") && parts.next().is_some() {
        Some(format!("Data/{}", path.replace('\\', "/")))
    } else {
        None
    }
}
