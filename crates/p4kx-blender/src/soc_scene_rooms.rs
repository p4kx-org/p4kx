// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocRoomIndex, SocRoomTarget, SocSourceGuid};
use p4kx_extraction::cryxml::SourceElement;
use p4kx_extraction::{SourceValue, SourcedValue};
use p4kx_model::{
    SocAreaComponent, SocAreaLink, SocRoomDeclaration, SocRoomRegistry, SocRoomSegment,
    SocSegmentGeometry, SocSegmentPoint,
};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_xml::XmlDocuments;

#[derive(Clone, Copy)]
pub(crate) struct IndexedRoom<'a> {
    pub(crate) index: &'a SocRoomIndex<'a>,
    pub(crate) origin: &'a XmlOrigin<'a>,
    pub(crate) root: SourceElement<'a>,
    pub(crate) complete: bool,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct RoomRegistry {
    xml: usize,
    rooms: Vec<Room>,
}

impl RoomRegistry {
    pub(super) fn read(
        registry: &SocRoomRegistry<'_>,
        origin: &XmlOrigin<'_>,
        documents: &mut XmlDocuments,
    ) -> Result<Self, SocSceneError> {
        Ok(Self {
            xml: documents.bind(registry.document(), origin)?,
            rooms: registry
                .rooms()
                .map(|room| Room::read(room, origin))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Room {
    ordinal: usize,
    position: SocXmlPosition,
    guid: SourcedValue<String>,
    bounds_center: SourcedValue<[f64; 3]>,
    pivot_relative_position: SourcedValue<[f64; 3]>,
    pivot_relative_rotation: SourcedValue<[f64; 4]>,
}

impl Room {
    fn read(room: SocRoomDeclaration<'_>, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        let element = room.element();
        Ok(Self {
            ordinal: room.ordinal(),
            position: SocXmlPosition::element(element.position())?,
            guid: origin.field(
                element,
                room.guid_attributes(),
                guid_value(room.guid_attributes()),
            )?,
            bounds_center: origin.named(element, "BoundsCenter", room.bounds_center())?,
            pivot_relative_position: origin.named(
                element,
                "PivotRelativePos",
                room.pivot_relative_position(),
            )?,
            pivot_relative_rotation: origin.named(
                element,
                "PivotRelativeRot",
                room.pivot_relative_rotation(),
            )?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct Area {
    ordinal: usize,
    position: SocXmlPosition,
    links: Vec<RoomLink>,
    segments: Vec<Segment>,
}

impl Area {
    pub(super) fn read(
        area: SocAreaComponent<'_>,
        registry: Option<IndexedRoom<'_>>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        Ok(Self {
            ordinal: area.ordinal(),
            position: SocXmlPosition::element(area.element().position())?,
            links: area
                .links()
                .map(|link| RoomLink::read(link, registry, origin))
                .collect::<Result<_, _>>()?,
            segments: area
                .segments()
                .map(|segment| Segment::read(segment, origin))
                .collect::<Result<_, _>>()?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct RoomLink {
    ordinal: (usize, usize),
    position: SocXmlPosition,
    guid: SourcedValue<String>,
    target: SourcedValue<RoomTarget>,
}

impl RoomLink {
    fn read(
        link: SocAreaLink<'_>,
        registry: Option<IndexedRoom<'_>>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = link.element();
        let guid = origin.field(
            element,
            link.guid_attributes(),
            guid_value(link.guid_attributes()),
        )?;
        Ok(Self {
            ordinal: link.ordinal(),
            position: SocXmlPosition::element(element.position())?,
            target: room_target(link, registry, origin)?,
            guid,
        })
    }
}

fn room_target(
    link: SocAreaLink<'_>,
    registry: Option<IndexedRoom<'_>>,
    origin: &XmlOrigin<'_>,
) -> Result<SourcedValue<RoomTarget>, SocSceneError> {
    use p4kx_extraction::SourceUseRole::{Payload, Selector};
    let mut uses = vec![origin.node(link.element(), Selector)?];
    for attribute in link.guid_attributes() {
        uses.push(origin.attribute(&attribute, Selector)?.1);
    }
    let source = match registry {
        Some(registry) => match registry.index.target(link) {
            SocRoomTarget::Absent => SourceValue::Absent,
            SocRoomTarget::Invalid => SourceValue::Invalid,
            SocRoomTarget::Unmatched { .. } => {
                uses.push(registry.origin.node(registry.root, Payload)?);
                SourceValue::Present(RoomTarget::Unmatched)
            }
            SocRoomTarget::Matched { room, .. } => {
                for attribute in room.guid_attributes() {
                    uses.push(registry.origin.attribute(&attribute, Payload)?.1);
                }
                SourceValue::Present(RoomTarget::Matched {
                    ordinal: room.ordinal(),
                })
            }
            SocRoomTarget::Ambiguous { rooms, .. } => {
                for room in rooms {
                    for attribute in room.guid_attributes() {
                        uses.push(registry.origin.attribute(&attribute, Payload)?.1);
                    }
                }
                SourceValue::Present(RoomTarget::Ambiguous {
                    ordinals: rooms.iter().map(|room| room.ordinal()).collect(),
                })
            }
            _ => SourceValue::Unsupported,
        },
        None => match guid_value(link.guid_attributes()) {
            SourceValue::Absent => SourceValue::Absent,
            SourceValue::Invalid => SourceValue::Invalid,
            _ => SourceValue::Present(RoomTarget::RegistryUnavailable),
        },
    };
    let complete = !matches!(source, SourceValue::Present(_))
        || registry.is_some_and(|registry| registry.complete);
    Ok(origin.sources.value(source, &uses, complete))
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum RoomTarget {
    RegistryUnavailable,
    Unmatched,
    Matched { ordinal: usize },
    Ambiguous { ordinals: Vec<usize> },
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Segment {
    ordinal: usize,
    position: SocXmlPosition,
    local_transform: SourcedValue<SegmentTransform>,
    geometry: SegmentGeometry,
}

#[derive(Debug, Serialize, JsonSchema)]
struct SegmentTransform {
    rotation_wxyz: [f64; 4],
    translation: [f64; 3],
}

impl Segment {
    fn read(segment: SocRoomSegment<'_>, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        let element = segment.element();
        let local_transform = origin
            .named(element, "transform_ls", segment.local_transform())?
            .map(|value| SegmentTransform {
                rotation_wxyz: value.rotation,
                translation: value.translation,
            });
        Ok(Self {
            ordinal: segment.ordinal(),
            position: SocXmlPosition::element(element.position())?,
            local_transform,
            geometry: SegmentGeometry::read(segment, origin)?,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum SegmentGeometry {
    Box {
        min: SourcedValue<[f64; 3]>,
        max: SourcedValue<[f64; 3]>,
    },
    Outline {
        height: SourcedValue<f64>,
        points: Vec<Point>,
    },
    TriangleSoup {
        points: Vec<Point>,
    },
    Other {
        label: SourcedValue<String>,
    },
}

impl SegmentGeometry {
    fn read(segment: SocRoomSegment<'_>, origin: &XmlOrigin<'_>) -> Result<Self, SocSceneError> {
        let element = segment.element();
        match segment.geometry() {
            SocSegmentGeometry::Box { min, max } => Ok(Self::Box {
                min: origin.named(element, "BoxMin", min)?,
                max: origin.named(element, "BoxMax", max)?,
            }),
            SocSegmentGeometry::Outline { height } => Ok(Self::Outline {
                height: origin.named(element, "Height", height)?,
                points: segment
                    .outline_points()
                    .map(|point| Point::read(point, "Pos", origin))
                    .collect::<Result<_, _>>()?,
            }),
            SocSegmentGeometry::TriangleSoup => Ok(Self::TriangleSoup {
                points: segment
                    .triangle_points()
                    .map(|point| Point::read(point, "pos", origin))
                    .collect::<Result<_, _>>()?,
            }),
            SocSegmentGeometry::Other { label } => Ok(Self::Other {
                label: origin.named(element, "Type", label)?.map(str::to_owned),
            }),
            _ => Ok(Self::Other {
                label: origin.field(element, std::iter::empty(), SourceValue::Unsupported)?,
            }),
        }
    }
}

#[derive(Debug, Serialize, JsonSchema)]
struct Point {
    ordinal: (Option<usize>, usize),
    position: SocXmlPosition,
    coordinates: SourcedValue<[f64; 3]>,
}

impl Point {
    fn read(
        point: SocSegmentPoint<'_>,
        attribute: &str,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = point.element();
        Ok(Self {
            ordinal: point.ordinal(),
            position: SocXmlPosition::element(element.position())?,
            coordinates: origin.named(element, attribute, point.position())?,
        })
    }
}

pub(super) fn guid_value<'document>(
    attributes: impl Iterator<Item = p4kx_extraction::cryxml::SourceAttribute<'document>>,
) -> SourceValue<String> {
    match SocSourceGuid::read(attributes) {
        SocSourceGuid::Absent => SourceValue::Absent,
        SocSourceGuid::Present(guid) => SourceValue::Present(guid.to_string()),
        _ => SourceValue::Invalid,
    }
}

pub(super) fn position(element: SourceElement<'_>) -> Result<SocXmlPosition, SocSceneError> {
    SocXmlPosition::element(element.position())
}
