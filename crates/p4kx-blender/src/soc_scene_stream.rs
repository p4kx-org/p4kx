// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_assembly::{SocEntityIndex, SocStreamInput};
use p4kx_extraction::{CancelToken, DecimalU64, EntryOrigin, SourceUses};
use p4kx_model::{SocEntityChunk, SocEntityFile};
use schemars::JsonSchema;
use serde::Serialize;

use crate::soc_scene::SocSceneError;
use crate::soc_scene_entities::Entity;
use crate::soc_scene_geometry::GeometryReference;
use crate::soc_scene_included::IncludedSources;
use crate::soc_scene_occurrence::{InputState, Member, Selector};
use crate::soc_scene_origin::{SocXmlPosition, XmlOrigin};
use crate::soc_scene_tile_ports::TilePorts;
use crate::soc_scene_xml::XmlDocuments;

pub(crate) struct IndexedStream<'a> {
    pub(crate) index: SocEntityIndex,
    pub(crate) classes: &'a crate::soc_scene_classes::Classes,
    pub(crate) admitted: BTreeMap<usize, &'a EntryOrigin>,
    pub(crate) complete: bool,
    pub(crate) room: Option<crate::soc_scene_rooms::IndexedRoom<'a>>,
    pub(crate) tile_ports: Option<&'a TilePorts<'a>>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Stream {
    selector: Selector,
    parent: Option<SocXmlPosition>,
    member: Member<Vec<Chunk>>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    included_objects: Vec<usize>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    geometry: Vec<GeometryReference>,
}

impl Stream {
    pub(crate) fn read(
        stream: &SocStreamInput,
        index: &IndexedStream<'_>,
        sources: &SourceUses,
        (documents, included): (&mut XmlDocuments<'_>, &mut IncludedSources),
        cancel: &CancelToken,
    ) -> Result<Self, SocSceneError> {
        let mut included_objects = Vec::new();
        let mut geometry = Vec::new();
        let member = match &stream.member {
            Some(selection) => Member::read(selection, sources, |bytes, origin| {
                let file = SocEntityFile::parse(bytes).map_err(|_| InputState::Invalid)?;
                included_objects = included
                    .bind(&file, origin, cancel)
                    .map_err(InputState::Capture)?;
                geometry
                    .try_reserve_exact(stream.geometry.len())
                    .map_err(|error| InputState::Capture(SocSceneError::Allocation(error)))?;
                for reference in &*stream.geometry {
                    crate::soc_scene_classes::check(cancel).map_err(InputState::Capture)?;
                    geometry.push(
                        GeometryReference::read(reference, origin).map_err(InputState::Capture)?,
                    );
                }
                file.entity_chunks()
                    .map(|chunk| match chunk {
                        Ok(chunk) => Chunk::read(&chunk, index, origin, documents)
                            .map_err(InputState::Capture),
                        Err(error) => Ok(Chunk::invalid(&error)),
                    })
                    .collect()
            })?,
            None => Member::UnusableSelector,
        };
        Ok(Self {
            selector: Selector::read(&stream.selector)?,
            parent: stream
                .parent
                .clone()
                .map(SocXmlPosition::element)
                .transpose()?,
            member,
            included_objects,
            geometry,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Chunk {
    Invalid {
        ordinal: Option<usize>,
    },
    Present {
        ordinal: usize,
        byte_offset: DecimalU64,
        byte_length: DecimalU64,
        xml: usize,
        entities: Vec<Entity>,
    },
}

impl Chunk {
    fn read(
        chunk: &SocEntityChunk<'_>,
        index: &IndexedStream<'_>,
        member: &XmlOrigin<'_>,
        documents: &mut XmlDocuments<'_>,
    ) -> Result<Self, SocSceneError> {
        let origin = XmlOrigin {
            sources: member.sources,
            scope: member.scope.clone(),
            chunk: Some(chunk.chunk_index()),
            archive_member: None,
        };
        let xml = documents.bind(chunk.document(), &origin)?;
        let entities = chunk
            .entities()
            .map(|entity| Entity::read(entity, index, &origin))
            .collect::<Result<_, _>>()?;
        Ok(Self::Present {
            ordinal: chunk.chunk_index(),
            byte_offset: DecimalU64::new(chunk.header().offset()),
            byte_length: DecimalU64::new(chunk.header().size()),
            xml,
            entities,
        })
    }

    const fn invalid(error: &p4kx_model::SocEntitySourceError) -> Self {
        use p4kx_model::SocEntitySourceError::{BigEndian, ChunkRange, Root, Xml};
        let ordinal = match error {
            BigEndian { chunk_index }
            | ChunkRange { chunk_index }
            | Root { chunk_index }
            | Xml { chunk_index, .. } => Some(*chunk_index),
            _ => None,
        };
        Self::Invalid { ordinal }
    }
}
