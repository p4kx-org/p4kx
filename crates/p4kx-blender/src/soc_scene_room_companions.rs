// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{SocContainerOccurrence, SocMemberSelection, SocSourceGuid};
use p4kx_extraction::cryxml::SourceDocument;
use p4kx_extraction::{CancelToken, SourceUses, SourcedValue};
use p4kx_model::SocRoomConnector;
use schemars::JsonSchema;
use serde::Serialize;

use super::{Member, companion_xml};
use crate::soc_scene::SocSceneError;
use crate::soc_scene_classes;
use crate::soc_scene_entities::EntityTarget;
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_rooms;
use crate::soc_scene_stream::IndexedStream;
use crate::soc_scene_xml::XmlDocuments;

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(super) struct RoomCompanions {
    room_mapping: Member<usize>,
    room_connectors: Member<RoomConnectors>,
}

impl RoomCompanions {
    pub(super) fn read(
        original: &SocContainerOccurrence,
        sources: &SourceUses,
        index: &IndexedStream<'_>,
        documents: &mut XmlDocuments<'_>,
        cancel: &CancelToken,
    ) -> Result<Option<Self>, SocSceneError> {
        if matches!(&original.room_mapping, SocMemberSelection::Missing)
            && matches!(&original.room_connectors, SocMemberSelection::Missing)
        {
            return Ok(None);
        }
        let room_mapping = read_member(
            &original.room_mapping,
            "RoomMapping",
            sources,
            documents,
            cancel,
            |xml, _, _| Ok(xml),
        )?;
        let room_connectors = read_member(
            &original.room_connectors,
            "EntityComponentRoomConnector",
            sources,
            documents,
            cancel,
            |xml, document, origin| RoomConnectors::read(xml, document, index, origin, cancel),
        )?;
        Ok(Some(Self {
            room_mapping,
            room_connectors,
        }))
    }
}

fn read_member<T>(
    selection: &SocMemberSelection,
    root: &str,
    sources: &SourceUses,
    documents: &mut XmlDocuments<'_>,
    cancel: &CancelToken,
    read: impl FnOnce(usize, &SourceDocument<'_>, &XmlOrigin<'_>) -> Result<T, SocSceneError>,
) -> Result<Member<T>, SocSceneError> {
    Member::read(selection, sources, |bytes, origin| {
        companion_xml::read(bytes, origin, root, documents, cancel, read)
    })
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct RoomConnectors {
    xml: usize,
    connectors: Vec<Connector>,
}

impl RoomConnectors {
    fn read(
        xml: usize,
        document: &SourceDocument<'_>,
        index: &IndexedStream<'_>,
        origin: &XmlOrigin<'_>,
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let connectors = SocRoomConnector::declarations(document.root())
            .map(|connector| {
                soc_scene_classes::check(cancel)?;
                Connector::read(connector, index, origin)
            })
            .collect::<Result<_, _>>()?;
        Ok(Self { xml, connectors })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
struct Connector {
    ordinal: usize,
    position: SocXmlPosition,
    cry_guid: SourcedValue<String>,
    owner: SourcedValue<EntityTarget>,
    pivot_relative_position: SourcedValue<[f64; 3]>,
    pivot_relative_rotation: SourcedValue<[f64; 4]>,
    cross_section_position: SourcedValue<[f64; 3]>,
    cross_section_rotation: SourcedValue<[f64; 4]>,
}

impl Connector {
    fn read(
        connector: SocRoomConnector<'_>,
        index: &IndexedStream<'_>,
        origin: &XmlOrigin<'_>,
    ) -> Result<Self, SocSceneError> {
        let element = connector.element();
        let guid = SocSourceGuid::read(connector.guid_attributes());
        Ok(Self {
            ordinal: connector.ordinal(),
            position: soc_scene_rooms::position(element)?,
            cry_guid: origin.field(
                element,
                connector.guid_attributes(),
                soc_scene_rooms::guid_value(connector.guid_attributes()),
            )?,
            owner: EntityTarget::read_guid(
                element,
                connector.guid_attributes(),
                guid,
                index,
                origin,
            )?,
            pivot_relative_position: origin.named(
                element,
                "PivotRelativePos",
                connector.pivot_relative_position(),
            )?,
            pivot_relative_rotation: origin.named(
                element,
                "PivotRelativeRot",
                connector.pivot_relative_rotation(),
            )?,
            cross_section_position: origin.named(
                element,
                "CrossSectionPos",
                connector.cross_section_position(),
            )?,
            cross_section_rotation: origin.named(
                element,
                "CrossSectionRot",
                connector.cross_section_rotation(),
            )?,
        })
    }
}
