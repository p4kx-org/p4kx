// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{
    SocChildInput, SocChildState, SocContainerOccurrence, SocEntityIndex, SocMemberSelection,
    SocMemberSelector, SocStreamInput,
};
use p4kx_extraction::{
    CancelToken, EntryOrigin, InputScope, PhysicalSourceRef, SourceInput, SourceUses, SourceValue,
    SourcedValue,
};
use p4kx_model::{SocContainerDescriptor, SocEntityFile, SocRoomRegistry};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes::Classes;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms::{IndexedRoom, RoomRegistry};
use crate::soc_scene_stream::{IndexedStream, Stream};
use crate::soc_scene_tile_ports::TilePorts;
use crate::soc_scene_xml::XmlDocuments;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Occurrence {
    placement: Option<(usize, usize)>,
    container: PhysicalSourceRef,
    descriptor: Member<usize>,
    registry: Member<RoomRegistry>,
    children: Vec<Child>,
    streams: Vec<Stream>,
}

impl Occurrence {
    pub(super) fn read(
        original: &SocContainerOccurrence,
        sources: &SourceUses,
        documents: &mut XmlDocuments<'_>,
        classes: &Classes,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let container = physical(&original.origin, sources)?;
        let descriptor_input = read_xml(&original.descriptor, sources, |bytes| {
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
        let room_input = read_xml(&original.registry, sources, |bytes| {
            SocRoomRegistry::parse(bytes).ok()
        })?;
        let registry = Member::read(&original.registry, sources, |_, _| {
            let (room, origin) = room_input.as_ref().ok_or(InputState::Invalid)?;
            RoomRegistry::read(room, origin, documents).map_err(InputState::Capture)
        })?;
        let tile_ports = descriptor_input
            .as_ref()
            .map(|(descriptor, origin)| TilePorts::new(descriptor, origin));
        let streams = streams(
            original,
            room_input.as_ref(),
            tile_ports.as_ref(),
            sources,
            documents,
            classes,
            cancel,
        )?;
        Ok(Self {
            placement: original.placement,
            container,
            descriptor,
            registry,
            children,
            streams,
        })
    }
}

fn read_xml<'a, T>(
    selection: &'a SocMemberSelection,
    sources: &'a SourceUses,
    parse: impl FnOnce(&'a [u8]) -> Option<T>,
) -> Result<Option<(T, XmlOrigin<'a>)>, SocSceneError> {
    let Some(member) = selection.selected() else {
        return Ok(None);
    };
    let Some(document) = member.payload().ok().and_then(parse) else {
        return Ok(None);
    };
    Ok(Some((
        document,
        XmlOrigin::new(sources, member.origin(), None)?,
    )))
}

fn streams(
    occurrence: &SocContainerOccurrence,
    room: Option<&(SocRoomRegistry<'_>, XmlOrigin<'_>)>,
    tile_ports: Option<&TilePorts<'_>>,
    sources: &SourceUses,
    documents: &mut XmlDocuments<'_>,
    classes: &Classes,
    cancel: &CancelToken,
) -> Result<Vec<Stream>, SocSceneError> {
    let (index, admitted, complete) = index_streams(&occurrence.streams, cancel)?;
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
        complete,
        classes,
        tile_ports,
        room: room_index.as_ref().map(|(index, origin, xml)| IndexedRoom {
            index,
            origin,
            root: xml.root(),
            complete: !xml.is_lossy(),
        }),
    };
    occurrence
        .streams
        .iter()
        .map(|declaration| {
            if cancel.is_cancelled() {
                return Err(SocSceneError::Cancelled);
            }
            Stream::read(declaration, &indexed, sources, documents)
        })
        .collect()
}

fn index_streams<'a>(
    declarations: &'a [SocStreamInput],
    cancel: &CancelToken,
) -> Result<(SocEntityIndex, BTreeMap<usize, &'a EntryOrigin>, bool), SocSceneError> {
    let mut by_member = BTreeMap::new();
    let mut complete = true;
    for declaration in declarations {
        if cancel.is_cancelled() {
            return Err(SocSceneError::Cancelled);
        }
        let Some(member) = declaration
            .member
            .as_ref()
            .and_then(SocMemberSelection::selected)
        else {
            complete = false;
            continue;
        };
        let physical_index = member.origin().entry().index();
        by_member.entry(physical_index).or_insert_with(|| {
            member
                .payload()
                .ok()
                .and_then(|bytes| SocEntityFile::parse(bytes).ok())
                .map(|file| (member.origin(), file))
        });
    }
    let mut index = SocEntityIndex::new();
    let mut admitted = BTreeMap::new();
    for (stream, parsed) in &by_member {
        let Some((origin, file)) = parsed else {
            complete = false;
            continue;
        };
        for chunk in file.entity_chunks() {
            if cancel.is_cancelled() {
                return Err(SocSceneError::Cancelled);
            }
            let Ok(chunk) = chunk else {
                complete = false;
                continue;
            };
            complete &= !chunk.document().is_lossy();
            index.add_chunk(*stream, &chunk);
        }
        admitted.insert(*stream, *origin);
    }
    Ok((index, admitted, complete))
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
    Ambiguous { candidates: Vec<PhysicalSourceRef> },
    Unreadable { source: PhysicalSourceRef },
    Invalid { source: PhysicalSourceRef },
    Present { source: PhysicalSourceRef, data: T },
}

pub(super) enum InputState {
    Invalid,
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
            SocMemberSelection::Selected(member) => {
                let origin = XmlOrigin::new(sources, member.origin(), None)?;
                let source = origin.source();
                let Ok(payload) = member.payload() else {
                    return Ok(Self::Unreadable { source });
                };
                match read(payload, &origin) {
                    Ok(data) => Ok(Self::Present { source, data }),
                    Err(InputState::Invalid) => Ok(Self::Invalid { source }),
                    Err(InputState::Capture(error)) => Err(error),
                }
            }
            _ => Err(SocSceneError::Position),
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
