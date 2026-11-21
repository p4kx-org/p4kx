// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    SocArchiveMember, SocChildInput, SocChildState, SocContainerOccurrence, SocMemberSelection,
    SocMemberSelector,
};
use p4kx_extraction::{
    CancelToken, EntryOrigin, InputScope, PhysicalSourceRef, SourceInput, SourceUses, SourceValue,
    SourcedValue,
};
use p4kx_model::{SocContainerDescriptor, SocRoomRegistry};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::Classes;
use crate::soc_scene_included::IncludedSources;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::{IndexedRoom, RoomRegistry};
use crate::soc_scene_stream::{IndexedStream, Stream};
use crate::soc_scene_tile_ports::TilePorts;
use crate::soc_scene_xml::XmlDocuments;

#[path = "soc_scene_companion_xml.rs"]
mod companion_xml;

#[path = "soc_scene_entity_reference_overrides.rs"]
mod entity_reference_overrides;

#[path = "soc_scene_room_companions.rs"]
mod room_companions;

#[path = "soc_scene_stream_index.rs"]
mod stream_index;

use entity_reference_overrides::EntityReferenceOverrideDocument;
use room_companions::RoomCompanions;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Occurrence {
    placement: Option<(usize, usize)>,
    container: PhysicalSourceRef,
    descriptor: Member<usize>,
    registry: Member<RoomRegistry>,
    children: Vec<Child>,
    streams: Vec<Stream>,
    #[serde(skip_serializing_if = "Option::is_none")]
    room_companions: Option<RoomCompanions>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    entity_reference_overrides: Vec<Member<EntityReferenceOverrideDocument>>,
}

impl Occurrence {
    pub(super) fn read(
        original: &SocContainerOccurrence,
        sources: &SourceUses,
        (documents, included): (&mut XmlDocuments<'_>, &mut IncludedSources),
        classes: &Classes,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let container = physical(&original.origin, sources)?;
        let descriptor_input =
            companion_xml::read_selected(&original.descriptor, sources, |bytes| {
                SocContainerDescriptor::parse(bytes).ok()
            })?;
        let mut children = Vec::new();
        let descriptor = Member::read(&original.descriptor, sources, |_, origin| {
            let (descriptor, _) = descriptor_input.as_ref().ok_or(InputState::Invalid)?;
            for (child, declaration) in original.children.iter().zip(descriptor.children()) {
                if child.selector.position != declaration.element().position() {
                    return Err(InputState::Capture(SocSceneError::Position));
                }
                children.push(
                    Child::read(child, declaration.element(), origin)
                        .map_err(InputState::Capture)?,
                );
            }
            if children.len() != original.children.len() {
                return Err(InputState::Capture(SocSceneError::Position));
            }
            documents
                .bind(descriptor.document(), origin)
                .map_err(InputState::Capture)
        })?;
        let room_input = companion_xml::read_selected(&original.registry, sources, |bytes| {
            SocRoomRegistry::parse(bytes).ok()
        })?;
        let registry = Member::read(&original.registry, sources, |_, _| {
            let (room, origin) = room_input.as_ref().ok_or(InputState::Invalid)?;
            RoomRegistry::read(room, origin, documents).map_err(InputState::Capture)
        })?;
        let tile_ports = descriptor_input
            .as_ref()
            .map(|(descriptor, origin)| TilePorts::new(descriptor, origin));
        let inputs = read_streams(
            original,
            room_input.as_ref(),
            tile_ports.as_ref(),
            sources,
            (documents, included),
            classes,
            cancel,
        )?;
        Ok(Self {
            placement: original.placement,
            container,
            descriptor,
            registry,
            children,
            streams: inputs.streams,
            room_companions: inputs.room_companions,
            entity_reference_overrides: inputs.entity_reference_overrides,
        })
    }
}

struct OccurrenceInputs {
    streams: Vec<Stream>,
    room_companions: Option<RoomCompanions>,
    entity_reference_overrides: Vec<Member<EntityReferenceOverrideDocument>>,
}

fn read_streams(
    occurrence: &SocContainerOccurrence,
    room: Option<&(SocRoomRegistry<'_>, XmlOrigin<'_>)>,
    tile_ports: Option<&TilePorts<'_>>,
    sources: &SourceUses,
    (documents, included): (&mut XmlDocuments<'_>, &mut IncludedSources),
    classes: &Classes,
    cancel: &CancelToken,
) -> Result<OccurrenceInputs, SocSceneError> {
    let (index, admitted, complete) = stream_index::read(&occurrence.streams, cancel)?;
    let room_index = room.map(|(registry, origin)| {
        (
            p4kx_assembly::SocRoomIndex::new(registry),
            origin,
            registry.document(),
        )
    });
    let indexed = IndexedStream {
        index,
        admitted,
        complete: complete && tile_ports.is_some(),
        classes,
        tile_ports,
        room: room_index.as_ref().map(|(index, origin, xml)| IndexedRoom {
            index,
            origin,
            root: xml.root(),
            complete: !xml.is_lossy(),
        }),
    };
    let streams = occurrence
        .streams
        .iter()
        .map(|declaration| {
            if cancel.is_cancelled() {
                return Err(SocSceneError::Cancelled);
            }
            Stream::read(
                declaration,
                &indexed,
                sources,
                (documents, included),
                cancel,
            )
        })
        .collect::<Result<_, _>>()?;
    let room_companions = RoomCompanions::read(occurrence, sources, &indexed, documents, cancel)?;
    let entity_reference_overrides =
        EntityReferenceOverrideDocument::read(occurrence, sources, &indexed, documents, cancel)?;
    Ok(OccurrenceInputs {
        streams,
        room_companions,
        entity_reference_overrides,
    })
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub(super) enum Member<T> {
    UnusableSelector,
    Missing,
    Ambiguous {
        candidates: Vec<PhysicalSourceRef>,
    },
    Unreadable {
        source: PhysicalSourceRef,
    },
    Invalid {
        source: PhysicalSourceRef,
    },
    Unsupported {
        source: PhysicalSourceRef,
        xml: usize,
    },
    Present {
        source: PhysicalSourceRef,
        data: T,
    },
}

pub(super) enum InputState {
    Invalid,
    Unsupported(usize),
    Capture(SocSceneError),
}

impl<T> Member<T> {
    pub(super) fn read(
        selection: &SocMemberSelection,
        sources: &SourceUses,
        read: impl FnOnce(&[u8], &XmlOrigin<'_>) -> Result<T, InputState>,
    ) -> Result<Self, SocSceneError> {
        match selection {
            SocMemberSelection::Missing => Ok(Self::Missing),
            SocMemberSelection::Ambiguous(origins) => Ok(Self::Ambiguous {
                candidates: physical_list(origins, sources)?,
            }),
            SocMemberSelection::Selected(member) => Self::read_physical(member, sources, read),
            _ => Err(SocSceneError::Position),
        }
    }

    pub(super) fn read_physical(
        member: &SocArchiveMember,
        sources: &SourceUses,
        read: impl FnOnce(&[u8], &XmlOrigin<'_>) -> Result<T, InputState>,
    ) -> Result<Self, SocSceneError> {
        let origin = XmlOrigin::new(sources, member.origin(), None)?;
        let source = origin.source();
        let Ok(payload) = member.payload() else {
            return Ok(Self::Unreadable { source });
        };
        match read(payload, &origin) {
            Ok(data) => Ok(Self::Present { source, data }),
            Err(InputState::Invalid) => Ok(Self::Invalid { source }),
            Err(InputState::Unsupported(xml)) => Ok(Self::Unsupported { source, xml }),
            Err(InputState::Capture(error)) => Err(error),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Child {
    selector: Selector,
    target: ChildTarget,
    local_transform: SourcedValue<[[f32; 4]; 3]>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ChildTarget {
    InvalidSelector,
    Missing,
    Ambiguous { candidates: Vec<PhysicalSourceRef> },
    Cycle { source: PhysicalSourceRef },
    Unreadable { source: PhysicalSourceRef },
    Opened { occurrence: usize },
}

impl Child {
    fn read(
        original: &SocChildInput,
        element: p4kx_extraction::cryxml::SourceElement<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let sources = origin.sources;
        let target = match &original.state {
            SocChildState::InvalidSelector => ChildTarget::InvalidSelector,
            SocChildState::Missing => ChildTarget::Missing,
            SocChildState::Ambiguous(origins) => ChildTarget::Ambiguous {
                candidates: physical_list(origins, sources)?,
            },
            SocChildState::Cycle(origin) => ChildTarget::Cycle {
                source: physical(origin, sources)?,
            },
            SocChildState::Unreadable { target, .. } => ChildTarget::Unreadable {
                source: physical(target, sources)?,
            },
            SocChildState::Opened { occurrence } => ChildTarget::Opened {
                occurrence: *occurrence,
            },
            _ => return Err(SocSceneError::Position),
        };
        Ok(Self {
            selector: Selector::read(&original.selector)?,
            target,
            local_transform: child_transform(element, origin)?,
        })
    }
}

fn child_transform(
    element: p4kx_extraction::cryxml::SourceElement<'_>,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<[[f32; 4]; 3]>, SocSceneError> {
    use p4kx_model::Transform;
    let names = ["pos", "rot"];
    let used = || {
        element.attributes().filter(|attr| {
            attr.namespace().is_none()
                && names
                    .iter()
                    .any(|name| attr.name().eq_ignore_ascii_case(name))
        })
    };
    let position = vector::<3>(element, "pos", [0.0; 3]);
    let rotation = vector::<4>(element, "rot", [1.0, 0.0, 0.0, 0.0]);
    let value = if used().next().is_none() {
        SourceValue::Absent
    } else {
        rotation
            .zip(position)
            .and_then(|(rotation, position)| Transform::from_quaternion(rotation, position))
            .map_or(SourceValue::Invalid, |transform| {
                SourceValue::Present(transform.rows)
            })
    };
    origin.field(element, used(), value)
}

fn vector<const N: usize>(
    element: p4kx_extraction::cryxml::SourceElement<'_>,
    name: &str,
    default: [f64; N],
) -> Option<[f64; N]> {
    let mut attributes = element
        .attributes()
        .filter(|attr| attr.namespace().is_none() && attr.name().eq_ignore_ascii_case(name));
    let Some(attribute) = attributes.next() else {
        return Some(default);
    };
    if attributes.next().is_some() {
        return None;
    }
    let mut parts = attribute.value().split(',');
    let mut values = [0.0; N];
    for value in &mut values {
        *value = parts.next()?.trim().parse().ok()?;
    }
    parts.next().is_none().then_some(values)
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Selector {
    ordinal: (usize, usize),
    position: SocXmlPosition,
}

impl Selector {
    pub(super) fn read(original: &SocMemberSelector) -> Result<Self, SocSceneError> {
        Ok(Self {
            ordinal: original.ordinal,
            position: SocXmlPosition::element(original.position.clone())?,
        })
    }
}

fn physical(input: &EntryOrigin, sources: &SourceUses) -> Result<PhysicalSourceRef, SocSceneError> {
    let scope = sources.observe(
        SourceInput::Archive(input),
        InputScope::ArchiveDirectoryMetadata,
    )?;
    Ok(scope.source_ref().clone())
}

fn physical_list(
    inputs: &[EntryOrigin],
    sources: &SourceUses,
) -> Result<Vec<PhysicalSourceRef>, SocSceneError> {
    inputs
        .iter()
        .map(|input| physical(input, sources))
        .collect()
}
