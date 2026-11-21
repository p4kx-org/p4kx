// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Read};
use std::ops::Range;

use p4kx_extraction::chunks::{ChunkFile, ivo};
use p4kx_extraction::{
    Archive, ArchiveWatch, EntryOrigin, InputScope, SourceInput, SourceLocation, SourceScope,
    SourceUse, SourceUseRole, SourceUses, SourceValue,
};
use p4kx_model::{
    NmcFrameField, NmcFrames, NmcTransformError, NmcTransforms, PhysicsBlob, PhysicsFloat,
    PhysicsGeometry, StatObjPhysics, StatObjPhysicsSlot,
};

use crate::collision_helper_document::{
    CollisionDisposition, CollisionSlot, ModelCollision, TriangleMesh,
};
use crate::{PackageError, PackageResult, PackageWriteContext};

pub(super) struct OriginalModels<'a, 'job> {
    pub(super) archive: &'a Archive,
    pub(super) watch: &'a ArchiveWatch,
    pub(super) evidence: &'a SourceUses,
    pub(super) context: &'a PackageWriteContext<'job>,
}

impl OriginalModels<'_, '_> {
    pub(super) fn bytes(&self, input: &EntryOrigin) -> PackageResult<Vec<u8>> {
        self.check()?;
        let entry = self
            .archive
            .entries()
            .get(input.entry().index())
            .filter(|_| self.archive.owns_origin(input))
            .ok_or_else(|| {
                error(io::Error::other(
                    "collision helper input has no owning Archive",
                ))
            })?;
        let mut reader = self.archive.open_entry_verified(entry).map_err(error)?;
        let size = usize::try_from(entry.uncompressed_size()).map_err(error)?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(size).map_err(error)?;
        let mut buffer = [0; 16 * 1024];
        loop {
            self.check()?;
            let count = reader.read(&mut buffer).map_err(error)?;
            if count == 0 {
                break;
            }
            let block = buffer
                .get(..count)
                .ok_or_else(|| error(io::Error::other("invalid Archive block length")))?;
            if count > size.saturating_sub(bytes.len()) {
                return Err(error(io::Error::other(
                    "collision helper input exceeds its stored size",
                )));
            }
            bytes.extend_from_slice(block);
        }
        if bytes.len() != size {
            return Err(error(io::Error::other(
                "collision helper input ended before its stored size",
            )));
        }
        self.check()?;
        Ok(bytes)
    }

    pub(super) fn observe(&self, input: &EntryOrigin) -> PackageResult<SourceScope> {
        if !self.archive.owns_origin(input) {
            return Err(error(io::Error::other(
                "collision evidence comes from another Archive",
            )));
        }
        self.evidence
            .observe(SourceInput::Archive(input), InputScope::Payload)
            .map_err(error)
    }

    pub(super) fn check(&self) -> PackageResult<()> {
        self.context.check()?;
        self.watch.poll().map_err(error)
    }

    pub(super) fn load(&self, input: &EntryOrigin) -> PackageResult<Option<ModelCollision>> {
        let bytes = self.bytes(input)?;
        let file = ChunkFile::parse(bytes).map_err(error)?;
        let mut chunks = file.chunks_of(ivo::STAT_OBJ_PHYSICS).peekable();
        if chunks.peek().is_none() {
            return Ok(None);
        }
        let scope = self.observe(input)?;
        let nodes = Nodes::read(&file, self.context)?;
        let mut slots = Vec::new();
        for (index, chunk) in chunks {
            self.check()?;
            let Ok(physics) = StatObjPhysics::read_chunk(&file, index) else {
                continue;
            };
            let source = SlotSource {
                scope: &scope,
                chunk: index,
                start: chunk.offset(),
                nodes: nodes.as_ref(),
            };
            for slot in physics.slots() {
                self.check()?;
                slots.push(source.slot(&slot, file.table_range(), self.evidence, self.context)?);
            }
        }
        Ok(Some(ModelCollision {
            source: scope.source_ref().clone(),
            slots,
        }))
    }
}

struct Nodes<'source> {
    frames: NmcFrames<'source>,
    computed: NmcTransforms<'source>,
    chunk: usize,
    start: u64,
}

impl<'source> Nodes<'source> {
    fn read(
        file: &'source ChunkFile<Vec<u8>>,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<Option<Self>> {
        let mut nodes = file.chunks_of(ivo::NODE_MESH_COMBO);
        let Some((index, chunk)) = nodes.next() else {
            return Ok(None);
        };
        if nodes.next().is_some() || !NmcFrames::supports(chunk) {
            return Ok(None);
        }
        let Some(frames) = file
            .chunk_data(index)
            .and_then(|bytes| NmcFrames::read(bytes).ok())
        else {
            return Ok(None);
        };
        let computed =
            NmcTransforms::read(frames, || context.check()).map_err(|cause| match cause {
                NmcTransformError::Allocation(cause) => error(cause),
                NmcTransformError::Checkpoint(cause) => cause,
                _ => error(io::Error::other("collision node lineage lost its source")),
            })?;
        Ok(Some(Self {
            frames,
            computed,
            chunk: index,
            start: chunk.offset(),
        }))
    }

    fn trace(
        &self,
        index: usize,
        scope: &SourceScope,
        uses: &mut Vec<SourceUse>,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<()> {
        let mut next = Some(index);
        for _ in 0..self.frames.len() {
            context.check()?;
            let Some(row) = next.and_then(|ordinal| self.frames.frame(ordinal)) else {
                break;
            };
            for (field, name) in [
                (NmcFrameField::LocalMatrix, "matrix"),
                (NmcFrameField::Scale, "scale"),
                (NmcFrameField::Parent, "parent"),
            ] {
                uses.push(at(
                    scope,
                    "model-physics-node",
                    &[self.chunk as u64, row.ordinal() as u64],
                    self.start,
                    row.range(field),
                    name,
                    SourceUseRole::Conversion,
                )?);
            }
            let parent = row.parent_word();
            next = (parent != u16::MAX).then_some(usize::from(parent));
        }
        Ok(())
    }
}

struct SlotSource<'a, 'source> {
    scope: &'a SourceScope,
    chunk: usize,
    start: u64,
    nodes: Option<&'a Nodes<'source>>,
}

impl SlotSource<'_, '_> {
    fn slot(
        &self,
        slot: &StatObjPhysicsSlot<'_>,
        table: Range<u64>,
        evidence: &SourceUses,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<CollisionSlot> {
        let coords = [self.chunk as u64, slot.ordinal() as u64];
        let key = self.at(&coords, slot.key_range(), "packedKey")?;
        let selector = at(
            self.scope,
            "model-chunk",
            &[],
            0,
            table,
            "chunkDirectory",
            SourceUseRole::Selector,
        )?;
        let mut frame_uses = vec![key.clone(), selector];
        let node = slot.node().map(usize::from);
        let frame = if let Some((node, nodes)) = node.zip(self.nodes) {
            nodes.trace(node, self.scope, &mut frame_uses, context)?;
            nodes.computed.frame(node).model.ok()
        } else {
            None
        };
        let declared = slot.declared_range();
        let (mesh, mut geometry_uses, disposition) =
            self.mesh(slot, &coords, declared.start, context)?;
        geometry_uses.push(key.clone());
        let disposition = match (disposition, node, frame) {
            (CollisionDisposition::Ready, None, _) => CollisionDisposition::WholeObject,
            (CollisionDisposition::Ready, Some(_), None) => CollisionDisposition::FrameUnavailable,
            (value, _, _) => value,
        };
        Ok(CollisionSlot {
            chunk: self.chunk,
            slot: slot.ordinal(),
            packed_key: evidence.value(SourceValue::Present(slot.key()), &[key], true),
            frame: evidence.value(
                frame.map_or(SourceValue::Unsupported, |frame| {
                    SourceValue::Present(frame.rows)
                }),
                &frame_uses,
                true,
            ),
            geometry: evidence.value(mesh, &geometry_uses, true),
            disposition,
        })
    }

    fn mesh(
        &self,
        slot: &StatObjPhysicsSlot<'_>,
        coords: &[u64],
        start: u64,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<(
        SourceValue<TriangleMesh>,
        Vec<SourceUse>,
        CollisionDisposition,
    )> {
        let mut uses = vec![self.at(coords, slot.length_range(), "length")?];
        let blob = slot.blob();
        let (geometry, disposition) = match blob {
            PhysicsBlob::Geometry(PhysicsGeometry::Trimesh(mesh)) => {
                uses.push(self.at(coords, start..start + mesh.index_range().end, "trimesh")?);
                let mut positions = Vec::new();
                let mut indices = Vec::new();
                positions
                    .try_reserve_exact(mesh.vertices().len())
                    .map_err(error)?;
                indices
                    .try_reserve_exact(mesh.triangles().len())
                    .map_err(error)?;
                for vertex in mesh.vertices() {
                    context.check()?;
                    positions.push(vertex.map(PhysicsFloat::value));
                }
                for triangle in mesh.triangles() {
                    context.check()?;
                    indices.push(triangle);
                }
                let disposition = if indices.is_empty() {
                    CollisionDisposition::EmptyMesh
                } else {
                    CollisionDisposition::Ready
                };
                (
                    SourceValue::Present(TriangleMesh {
                        positions_z_up_metres: positions,
                        indices,
                    }),
                    disposition,
                )
            }
            PhysicsBlob::Geometry(_) => {
                (SourceValue::Unsupported, CollisionDisposition::NonMeshShape)
            }
            PhysicsBlob::MissingPayload | PhysicsBlob::Malformed(_) => {
                (SourceValue::Invalid, CollisionDisposition::MalformedBlob)
            }
            _ => (
                SourceValue::Unsupported,
                CollisionDisposition::UnsupportedBlob,
            ),
        };
        if let Some(bytes) = slot.bytes()
            && !matches!(&geometry, SourceValue::Present(_))
        {
            uses.push(self.at(coords, start..start + bytes.len() as u64, "observedBlob")?);
        }
        Ok((geometry, uses, disposition))
    }

    fn at(&self, coordinates: &[u64], range: Range<u64>, field: &str) -> PackageResult<SourceUse> {
        at(
            self.scope,
            "model-physics-slot",
            coordinates,
            self.start,
            range,
            field,
            SourceUseRole::Payload,
        )
    }
}

fn at(
    scope: &SourceScope,
    namespace: &str,
    coordinates: &[u64],
    base: u64,
    range: Range<u64>,
    field: &str,
    role: SourceUseRole,
) -> PackageResult<SourceUse> {
    let indices = coordinates
        .iter()
        .copied()
        .chain([base + range.start, base + range.end])
        .collect();
    scope
        .at(
            SourceLocation::Subresource {
                namespace: namespace.to_owned(),
                indices,
                field: field.to_owned(),
            },
            role,
        )
        .map_err(error)
}

pub(super) fn error(cause: impl std::error::Error + Send + Sync + 'static) -> PackageError {
    PackageError::CollisionEvidence(Box::new(cause))
}
