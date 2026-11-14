// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::cryxml::{
    SourceAttribute, SourceDeclaration, SourceDeclarationKind, SourceElement,
};
use p4kx_extraction::{DecimalU64, SourceUseRole};

use super::links::{self, ReferenceSelection};
use super::origins;
use super::payload::{self as wire, DefinitionFamily, DefinitionPayload};
use super::projection::{DefinitionProjection, Failure};
use super::timeline_payload::{Event, EventAttribute, EventField, EventTimeline, TimelineTarget};
use crate::audio_control_payload::AtlPosition;
use crate::audio_record_origins as records;
use crate::xml_source_projection::declaration_position;
use crate::{MediaError, MediaKind, MediaWriter, Status, Subresource};

#[derive(Clone, Copy, Default)]
pub(super) enum TimelineNode {
    #[default]
    Other,
    List,
    Animation(TimelineIdentity),
    Event(TimelineIdentity),
}

#[derive(Clone, Copy)]
pub(super) struct TimelineIdentity {
    pub(super) index: u64,
    pub(super) position: AtlPosition,
}

impl TimelineNode {
    pub(super) fn at(
        parent: Self,
        family: DefinitionFamily,
        element: SourceElement<'_>,
        parent_index: Option<u64>,
        identity: TimelineIdentity,
    ) -> Self {
        if !matches!(family, DefinitionFamily::Animevents) || element.namespace().is_some() {
            return Self::Other;
        }
        match (parent, element.name(), parent_index) {
            (_, "anim_event_list", None) => Self::List,
            (Self::List, "animation", Some(_)) => Self::Animation(identity),
            (Self::Animation(animation), "event", Some(_)) => Self::Event(animation),
            _ => Self::Other,
        }
    }

    pub(super) fn write(
        self,
        projection: &DefinitionProjection<'_, '_>,
        element: SourceElement<'_>,
        index: u64,
        position: AtlPosition,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        match self {
            Self::Animation(_) => write_animation(projection, element, index, position, writer)?,
            Self::Event(animation) => write_event(projection, animation, index, position, writer)?,
            Self::Other | Self::List => {}
        }
        Ok(())
    }

    pub(super) fn write_attribute(
        self,
        projection: &DefinitionProjection<'_, '_>,
        attribute: &SourceDeclaration<'_>,
        indices: [u64; 2],
        position: AtlPosition,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), Failure> {
        let Self::Event(_) = self else {
            return Ok(());
        };
        let SourceDeclarationKind::Attribute { namespace } = attribute.kind() else {
            return Ok(());
        };
        projection.write(
            position,
            wire::EVENT_ATTRIBUTE,
            indices.to_vec(),
            &DefinitionPayload::EventAttribute(EventAttribute {
                field: EventField::of(attribute.qualified_name(), namespace),
                qualified_name: attribute.qualified_name(),
                namespace,
                value: attribute.value(),
                lexeme: attribute.lexeme(),
                position,
            }),
            writer,
        )?;
        Ok(())
    }
}

fn write_event(
    projection: &DefinitionProjection<'_, '_>,
    animation: TimelineIdentity,
    index: u64,
    position: AtlPosition,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let proof = origins::position(projection.input, position, SourceUseRole::Payload, writer)?;
    let mut prepared = origins::record(projection.input, proof.clone(), writer)?;
    let mut parent = writer.capture_origins();
    parent.include(&proof, writer)?;
    parent.include(
        &origins::position(
            projection.input,
            animation.position,
            SourceUseRole::Selector,
            writer,
        )?,
        writer,
    )?;
    records::field(
        &mut prepared,
        "/data/payload/animation_element",
        parent.finish(),
        writer,
    )?;
    prepared.write(
        writer,
        &Subresource::new(wire::EVENT, vec![index]),
        Status::Complete,
        &DefinitionPayload::Event(Event {
            animation_element: DecimalU64::new(animation.index),
            position,
        }),
        &[],
    )
}

fn write_animation(
    projection: &DefinitionProjection<'_, '_>,
    element: SourceElement<'_>,
    index: u64,
    position: AtlPosition,
    writer: &mut MediaWriter<'_>,
) -> Result<(), Failure> {
    let element_proof =
        origins::position(projection.input, position, SourceUseRole::Payload, writer)?;
    let mut prepared = origins::record(projection.input, element_proof, writer)?;
    let mut names = element
        .attributes()
        .filter(|attribute| attribute.namespace().is_none() && attribute.name() == "name");
    let first = names.next();
    let target = match (first, names.next()) {
        (Some(attribute), None) => target_reference(projection, &attribute, &mut prepared, writer)?,
        (Some(_), Some(_)) => TimelineTarget::AmbiguousName,
        (None, _) => TimelineTarget::MissingName,
    };
    prepared.write(
        writer,
        &Subresource::new(wire::EVENT_TIMELINE, vec![index]),
        Status::Complete,
        &DefinitionPayload::EventTimeline(EventTimeline { position, target }),
        &[],
    )?;
    Ok(())
}

fn target_reference<'source>(
    projection: &DefinitionProjection<'_, '_>,
    attribute: &SourceAttribute<'source>,
    prepared: &mut crate::origin_prepared::PreparedRecordOrigins,
    writer: &mut MediaWriter<'_>,
) -> Result<TimelineTarget<'source>, Failure> {
    let position = declaration_position(attribute.position())?;
    let proof = origins::position(projection.input, position, SourceUseRole::Selector, writer)?;
    let authored_path = attribute.value();
    let archive_path = authored_path
        .rsplit_once('.')
        .filter(|(_, extension)| extension.eq_ignore_ascii_case("caf"))
        .and_then(|_| links::archive_animation_path(authored_path));
    for field in ["authored_path", "archive_path", "position"] {
        records::subtree(
            prepared,
            &format!("/data/payload/target/{field}"),
            proof.clone(),
            writer,
        )?;
    }
    let reference = ReferenceSelection {
        authored_path,
        archive_path: archive_path.as_deref(),
        kind: Some(MediaKind::Animation),
        original: &proof,
        selector: &proof,
    }
    .resolve_at(
        projection.context.directory,
        "/data/payload/target/reference",
        prepared,
        writer,
    )?;
    Ok(TimelineTarget::Path {
        authored_path,
        archive_path,
        position,
        reference: Box::new(reference),
    })
}
