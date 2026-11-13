// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourceUseRole;
use p4kx_extraction::chunks::{ChunkDirectory, ivo};
use p4kx_extraction::model::{
    NmcFrame, NmcFrameField, NmcFrames, NmcTransforms, StatObjPhysicsSlot,
};

use super::evidence::{PhysicsOrigins, bind};
use super::frame_hierarchy::{self, FrameDisposition, PhysicsFrame};
use super::payload::{FRAME, NODE, PhysicsNode, PhysicsPayload};
use crate::origin_contributions::OriginContributions;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::provider::model_source::origins::{self, ModelOrigins};
use crate::required_reader::RequiredReader;
use crate::{MediaError, MediaWriter, Status, Subresource};

pub(in crate::provider::model_source) struct NodeFrames {
    owner: Option<usize>,
    offset: u64,
    disposition: FrameDisposition,
    source: OriginContributions,
    bytes: Vec<u8>,
}

impl NodeFrames {
    pub(in crate::provider::model_source) fn load(
        file: &ChunkDirectory,
        reader: &mut RequiredReader<'_>,
        origins: &ModelOrigins<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let membership = origins.payload(
            file.table_range(),
            "modelNmcDirectory",
            SourceUseRole::Selector,
            writer,
        )?;
        let mut candidates = file
            .chunks()
            .iter()
            .enumerate()
            .filter(|(_, chunk)| chunk.chunk_type() == ivo::NODE_MESH_COMBO);
        let mut loaded = Self {
            owner: None,
            offset: 0,
            disposition: FrameDisposition::MissingNodeTable,
            source: membership,
            bytes: Vec::new(),
        };
        let Some((index, chunk)) = candidates.next() else {
            return Ok(loaded);
        };
        if candidates.next().is_some() {
            loaded.disposition = FrameDisposition::AmbiguousNodeTable;
            return Ok(loaded);
        }
        let row = file.table_row(index).ok_or_else(|| {
            MediaError::Contract("NMC frame lost the physical chunk table row".into())
        })?;
        let mut source = writer.capture_origins();
        source.include_original(&loaded.source)?;
        source.include_original(&origins.payload(
            row.version,
            "modelNmcTableVersion",
            SourceUseRole::Selector,
            writer,
        )?)?;
        loaded.source = source.finish();
        loaded.owner = Some(index);
        loaded.offset = chunk.offset();
        if !NmcFrames::supports(chunk) {
            loaded.disposition = FrameDisposition::UnsupportedNodeTable;
            return Ok(loaded);
        }
        let bytes = reader.read_chunk(chunk.offset()..chunk.offset() + chunk.size())?;
        loaded.disposition = if NmcFrames::read(&bytes).is_ok() {
            FrameDisposition::Resolved
        } else {
            FrameDisposition::MalformedNodeTable
        };
        loaded.bytes = bytes;
        Ok(loaded)
    }

    pub(in crate::provider::model_source) const fn original_bytes(
        &self,
        index: usize,
    ) -> Option<&[u8]> {
        match self.owner {
            Some(owner)
                if owner == index
                    && !matches!(self.disposition, FrameDisposition::UnsupportedNodeTable) =>
            {
                Some(self.bytes.as_slice())
            }
            _ => None,
        }
    }

    pub(in crate::provider::model_source) fn project<'data>(
        &'data self,
        origins: &ModelOrigins<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<FrameProjection<'data>, MediaError> {
        let table = self
            .owner
            .zip(NmcFrames::read(&self.bytes).ok())
            .map(|(chunk, frames)| {
                let source = frame_hierarchy::read(frames, writer)?;
                for node in frames.frames() {
                    writer.checkpoint()?;
                    self.node(chunk, node, origins, writer)?;
                }
                Ok::<_, MediaError>((chunk, frames, source))
            })
            .transpose()?;
        Ok(FrameProjection {
            table,
            loaded: self,
        })
    }

    fn node(
        &self,
        chunk: usize,
        node: NmcFrame<'_>,
        origins: &ModelOrigins<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = origins.prepared()?;
        origins::at(
            &mut prepared,
            "/subresource/indices",
            self.count_evidence(origins, writer)?,
            writer,
        )?;
        for (name, field) in [
            ("stored_id", NmcFrameField::StoredId),
            ("parent_word", NmcFrameField::Parent),
            ("geometry_type", NmcFrameField::GeometryType),
            ("local_matrix_bits", NmcFrameField::LocalMatrix),
            ("companion_matrix_bits", NmcFrameField::CompanionMatrix),
            ("scale_bits", NmcFrameField::Scale),
        ] {
            let source = self.field(origins, node, field, name, SourceUseRole::Payload, writer)?;
            bind(&mut prepared, name, source, writer)?;
        }
        prepared.write(
            writer,
            &Subresource::new(NODE, vec![chunk as u64, node.ordinal() as u64]),
            Status::Complete,
            &PhysicsPayload::Node(PhysicsNode {
                stored_id: node.stored_id(),
                parent_word: node.parent_word(),
                geometry_type: node.geometry_type(),
                local_matrix_bits: node.local_matrix_bits(),
                companion_matrix_bits: node.companion_matrix_bits(),
                scale_bits: node.scale_bits(),
            }),
            &[],
        )
    }

    fn count_evidence(
        &self,
        origins: &ModelOrigins<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let range = NmcFrames::count_range();
        let mut evidence = writer.capture_origins();
        evidence.include_original(&self.source)?;
        evidence.include_original(&origins.payload(
            self.offset + range.start..self.offset + range.end,
            "modelNmcNodeCount",
            SourceUseRole::Selector,
            writer,
        )?)?;
        Ok(evidence.finish())
    }

    fn field(
        &self,
        origins: &ModelOrigins<'_>,
        node: NmcFrame<'_>,
        field: NmcFrameField,
        name: &str,
        role: SourceUseRole,
        writer: &MediaWriter<'_>,
    ) -> Result<OriginContributions, MediaError> {
        let range = node.range(field);
        origins.payload(
            self.offset + range.start..self.offset + range.end,
            name,
            role,
            writer,
        )
    }
}

pub(in crate::provider::model_source) struct FrameProjection<'data> {
    table: Option<(usize, NmcFrames<'data>, NmcTransforms<'data>)>,
    loaded: &'data NodeFrames,
}

impl FrameProjection<'_> {
    pub(super) fn slot(
        &self,
        chunk: usize,
        slot: &StatObjPhysicsSlot<'_>,
        original: &PhysicsOrigins<'_, '_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let key = original.use_bytes(slot.key_range(), "physicsPackedKey", writer)?;
        let mut prepared = original.model.prepared()?;
        origins::at(&mut prepared, "/subresource/indices", key.clone(), writer)?;
        let frame = match slot.node() {
            None => PhysicsFrame {
                disposition: FrameDisposition::WholeObject,
                node_chunk: None,
                local: None,
                entity: None,
            },
            Some(index) => self.node_frame(
                usize::from(index),
                &key,
                original.model,
                &mut prepared,
                writer,
            )?,
        };
        if slot.node().is_none() {
            origins::at(&mut prepared, "/data/payload", key, writer)?;
        }
        prepared.write(
            writer,
            &Subresource::new(FRAME, vec![chunk as u64, slot.ordinal() as u64]),
            Status::Complete,
            &PhysicsPayload::Frame(frame),
            &[],
        )
    }

    fn node_frame(
        &self,
        index: usize,
        key: &OriginContributions,
        origins: &ModelOrigins<'_>,
        prepared: &mut PreparedRecordOrigins,
        writer: &MediaWriter<'_>,
    ) -> Result<PhysicsFrame, MediaError> {
        let loaded = self.loaded;
        let mut capture = writer.capture_origins();
        capture.include_original(key)?;
        capture.include_original(&loaded.source)?;
        let Some((chunk, frames, hierarchy)) = &self.table else {
            if !loaded.bytes.is_empty() {
                capture.include_original(&origins.payload(
                    loaded.offset..loaded.offset + loaded.bytes.len() as u64,
                    "modelNmcReadFailure",
                    SourceUseRole::Selector,
                    writer,
                )?)?;
            }
            origins::at(prepared, "/data/payload", capture.finish(), writer)?;
            return Ok(PhysicsFrame {
                disposition: loaded.disposition,
                node_chunk: loaded.owner,
                local: None,
                entity: None,
            });
        };
        capture.include_original(&loaded.count_evidence(origins, writer)?)?;
        let selection = capture.finish();
        bind(prepared, "node_chunk", selection.clone(), writer)?;
        let (local, world) = self.chain_evidence(*frames, index, &selection, origins, writer)?;
        bind(prepared, "local", local, writer)?;
        bind(prepared, "entity", world.clone(), writer)?;
        bind(prepared, "disposition", world, writer)?;
        Ok(frame_hierarchy::frame(hierarchy, *chunk, index))
    }

    fn chain_evidence(
        &self,
        frames: NmcFrames<'_>,
        index: usize,
        selection: &OriginContributions,
        origins: &ModelOrigins<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<(OriginContributions, OriginContributions), MediaError> {
        let mut route = writer.capture_origins();
        route.include_original(selection)?;
        let mut local = writer.capture_origins();
        local.include_original(selection)?;
        let mut next = Some(index);
        for depth in 0..frames.len() {
            writer.checkpoint()?;
            let Some(node) = next.and_then(|index| frames.frame(index)) else {
                break;
            };
            for (name, field) in [
                ("modelNmcLocalMatrix", NmcFrameField::LocalMatrix),
                ("modelNmcScale", NmcFrameField::Scale),
                ("modelNmcParent", NmcFrameField::Parent),
            ] {
                let evidence = self.loaded.field(
                    origins,
                    node,
                    field,
                    name,
                    SourceUseRole::Selector,
                    writer,
                )?;
                route.include_original(&evidence)?;
                if depth == 0 && !matches!(field, NmcFrameField::Parent) {
                    local.include_original(&evidence)?;
                }
            }
            let parent = node.parent_word();
            next = (parent != u16::MAX).then_some(usize::from(parent));
        }
        Ok((local.finish(), route.finish()))
    }
}
