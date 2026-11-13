// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::ops::Range;
use std::path::Path;

use p4kx_extraction::{
    DecimalU64, EntryOrigin, InputScope, ModelArchiveSource, OriginArtifactDigest,
    OriginDigestWriter, SourceInput, SourceLocation, SourceScope, SourceUse, SourceUseRole,
    SourceUses,
};
use p4kx_gltf::MeshVertexMap;
use p4kx_model::{MeshOrigin, Model, ModelChunk};

use crate::mesh_vertex_map_wire::{self as wire, Mapping, Row};
use crate::package_receipt::PackageFile;
use crate::{MESHES_FOLDER, PackageError, PackageResult, PackageWriteContext};

const VERTICES_PER_USE_BATCH: u64 = 4096;

pub(super) struct Writer {
    sink: OriginDigestWriter<BufWriter<std::fs::File>>,
    ownership: p4kx_guard::OwnedFileToken,
    inventory: SourceUses,
    meshes: u64,
    vertices: u64,
    uses: u64,
}

impl Writer {
    pub(super) fn new(path: &Path, context: &PackageWriteContext<'_>) -> PackageResult<Self> {
        let (file, ownership) = context.create_file(path)?.into_parts();
        let mut writer = Self {
            sink: OriginDigestWriter::new(BufWriter::new(file)),
            ownership,
            inventory: SourceUses::with_batched_use_refs(),
            meshes: 0,
            vertices: 0,
            uses: 0,
        };
        writer
            .row(&Row::Header {
                format: wire::FORMAT,
                version: wire::VERSION,
            })
            .map_err(|cause| PackageError::write(path, cause))?;
        Ok(writer)
    }

    pub(super) fn observe(
        &mut self,
        map: &MeshVertexMap<'_>,
        model: &Model,
        sources: &[ModelArchiveSource],
        context: &PackageWriteContext<'_>,
    ) -> io::Result<()> {
        let evidence = source_of(model, sources, map.origin);
        let scope = evidence
            .map(|(entry, chunk)| {
                Ok::<_, io::Error>(Bound {
                    scope: self
                        .inventory
                        .observe(SourceInput::Archive(entry), InputScope::Payload)
                        .map_err(io::Error::other)?,
                    chunk,
                    complete: entry.ancestors().all(|step| step.is_some()),
                })
            })
            .transpose()?;
        let anchors = scope
            .as_ref()
            .map(|bound| {
                bound.use_range(
                    bound.chunk.layout.offset..bound.chunk.layout.offset + bound.chunk.layout.size,
                    "meshChunk",
                    SourceUseRole::Selector,
                )
            })
            .transpose()?;
        let origin = self.inventory.origin(
            anchors.as_slice(),
            scope.as_ref().is_some_and(|scope| scope.complete),
        );
        let mapping = if map.source_vertices.is_some() {
            Mapping::Present
        } else {
            Mapping::Unknown
        };
        self.row(&Row::Mesh {
            mesh: map.mesh,
            position_accessor: map.position_accessor,
            vertices: DecimalU64::new(map.vertices),
            model_node: map.model_node,
            requested_lod: map.requested_lod,
            selected_lod: map.model_lod,
            geometry_lod: map.origin.map(|origin| origin.lod),
            geometry_chunk: map.origin.map(|origin| origin.chunk),
            mapping,
            origin: &origin,
        })?;
        self.meshes += 1;
        let mut vertices = 0_u64;
        for vertex in map.vertex_origins() {
            context.check().map_err(io::Error::other)?;
            self.vertex(map.mesh, vertex, scope.as_ref())?;
            vertices += 1;
        }
        if map.source_vertices.is_some() && vertices != map.vertices {
            return Err(io::Error::other("incomplete first-use vertex map"));
        }
        self.vertices += vertices;
        self.finish_uses()
    }

    fn vertex(
        &mut self,
        mesh: usize,
        vertex: p4kx_model::MeshVertexOrigin,
        bound: Option<&Bound<'_>>,
    ) -> io::Result<()> {
        let own = bound.map(|bound| bound.vertex(vertex)).transpose()?;
        let uses = own.as_ref().map_or(&[][..], |uses| uses.as_slice());
        let origin = self
            .inventory
            .origin(uses, bound.is_some_and(|bound| bound.complete));
        self.row(&Row::Vertex {
            mesh,
            vertex: vertex.output_vertex,
            source_vertex: vertex.source_vertex,
            origin: &origin,
        })?;
        if (u64::from(vertex.output_vertex) + 1).is_multiple_of(VERTICES_PER_USE_BATCH) {
            self.finish_uses()?;
        }
        Ok(())
    }

    fn finish_uses(&mut self) -> io::Result<()> {
        self.uses += self.inventory.drain_use_batch(|source_use| {
            write_row(&mut self.sink, &Row::SourceUse { source_use })
        })?;
        Ok(())
    }

    fn row(&mut self, row: &Row<'_>) -> io::Result<()> {
        write_row(&mut self.sink, row)
    }

    pub(super) fn finish(
        mut self,
        path: &Path,
        name: &str,
        glb: &str,
        glb_digest: &OriginArtifactDigest,
        context: &PackageWriteContext<'_>,
    ) -> PackageResult<PackageFile> {
        let inventory = self
            .inventory
            .freeze()
            .map_err(|error| PackageError::write(path, io::Error::other(error)))?;
        for source in inventory.sources() {
            self.row(&Row::Source { source })
                .map_err(|error| PackageError::write(path, error))?;
        }
        self.row(&Row::Artifact {
            glb,
            artifact: glb_digest,
        })
        .map_err(|error| PackageError::write(path, error))?;
        self.row(&Row::End {
            meshes: DecimalU64::new(self.meshes),
            vertices: DecimalU64::new(self.vertices),
            sources: DecimalU64::new(inventory.sources().len() as u64),
            source_uses: DecimalU64::new(self.uses),
        })
        .map_err(|error| PackageError::write(path, error))?;
        context.check()?;
        self.ownership
            .ensure_writable(path, context.cancel_flag())?;
        let (digest, sink) = self
            .sink
            .finish_with_sink()
            .map_err(|error| PackageError::write(path, error))?;
        sink.get_ref()
            .sync_all()
            .map_err(|error| PackageError::write(path, error))?;
        Ok(PackageFile::new(format!("{MESHES_FOLDER}/{name}"), &digest))
    }
}

fn write_row(writer: &mut impl Write, row: &Row<'_>) -> io::Result<()> {
    serde_json::to_writer(&mut *writer, row)?;
    writer.write_all(b"\n")
}

fn source_of<'a>(
    model: &'a Model,
    sources: &'a [ModelArchiveSource],
    origin: Option<MeshOrigin>,
) -> Option<(&'a EntryOrigin, &'a ModelChunk)> {
    let MeshOrigin {
        file, chunk: slot, ..
    } = origin?;
    let entry = sources
        .iter()
        .find(|source| source.file() == file)?
        .origin()?;
    let chunk = model
        .chunks
        .iter()
        .find(|chunk| chunk.file == file && chunk.index == slot)?;
    Some((entry, chunk))
}

struct Bound<'a> {
    scope: SourceScope,
    chunk: &'a ModelChunk,
    complete: bool,
}

impl Bound<'_> {
    fn vertex(&self, vertex: p4kx_model::MeshVertexOrigin) -> io::Result<[SourceUse; 3]> {
        let base = self.chunk.layout.offset;
        let absolute = |span: Range<u64>| base + span.start..base + span.end;
        Ok([
            self.use_range(
                absolute(vertex.index_range()),
                "indexWord",
                SourceUseRole::Payload,
            )?,
            self.use_range(
                absolute(vertex.page_base_range()),
                "vertexPageBase",
                SourceUseRole::Payload,
            )?,
            self.use_range(
                absolute(vertex.index_selection_range()),
                "indexSelection",
                SourceUseRole::Selector,
            )?,
        ])
    }

    fn use_range(
        &self,
        span: Range<u64>,
        field: &str,
        role: SourceUseRole,
    ) -> io::Result<SourceUse> {
        let start = self.chunk.layout.offset;
        if span.start < start || span.end > start + self.chunk.layout.size || span.start >= span.end
        {
            return Err(io::Error::other(
                "mesh evidence crosses the selected geometry chunk",
            ));
        }
        self.scope
            .at(
                SourceLocation::Subresource {
                    namespace: "model-byte-range".to_owned(),
                    indices: vec![span.start, span.end],
                    field: field.to_owned(),
                },
                role,
            )
            .map_err(io::Error::other)
    }
}
