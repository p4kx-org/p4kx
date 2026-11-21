// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io;

use gltf_json::{Mesh as GltfMesh, Root};
use p4kx_model::{Mesh, MeshSubmeshSource, ModelChunk};

use crate::instance::Instance;
use crate::layout::MeshLayout;
use crate::materials::{MaterialPlan, Slot};

/// The material choice that supplied an emitted primitive before global GLB material sharing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum PrimitiveMaterialSource {
    /// The original zero-based model material library slot selected by the renderer.
    Library(usize),
    /// The source provider supplied the fallback material.
    SuppliedFallback,
    /// The built-in fallback material supplied the primitive.
    BuiltInFallback,
    /// No material was emitted on this primitive.
    Unassigned,
}

/// One observed primitive indexed in the actual GLB mesh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct MeshPrimitiveMap {
    /// Zero-based primitive position in the emitted GLB mesh.
    pub primitive: usize,
    /// The emitted index accessor.
    pub index_accessor: Option<usize>,
    /// The emitted dense GLB material array index.
    pub material_index: Option<usize>,
    /// The draw material selected before equal GLB materials were shared.
    pub material_source: PrimitiveMaterialSource,
    /// The submesh ordinal within the decoded, grouped model mesh.
    pub model_submesh: usize,
    /// Original IVO source row and decoded-file coordinates when observed.
    pub original: Option<MeshSubmeshSource>,
}

#[derive(Clone, Copy)]
pub(crate) struct ObservedMesh<'a> {
    written: &'a GltfMesh,
    planned: &'a MeshLayout,
    materials: &'a MaterialPlan<'a>,
    chunk: Option<&'a ModelChunk>,
}

impl std::fmt::Debug for ObservedMesh<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObservedMesh")
            .field("primitives", &self.written.primitives.len())
            .finish_non_exhaustive()
    }
}

impl<'a> ObservedMesh<'a> {
    pub(crate) fn new(
        document: &Root,
        written: &'a GltfMesh,
        planned: &'a MeshLayout,
        instance: &'a Instance<'a>,
        mesh: &Mesh,
    ) -> io::Result<Self> {
        if written.primitives.len() != planned.primitives.len()
            || written.primitives.iter().any(|primitive| {
                primitive
                    .indices
                    .is_none_or(|index| index.value() >= document.accessors.len())
                    || primitive
                        .material
                        .is_some_and(|index| index.value() >= document.materials.len())
            })
        {
            return Err(io::Error::other(
                "GLB primitives and source layout disagree",
            ));
        }
        let chunk = mesh.origin().and_then(|origin| {
            instance
                .model
                .chunks
                .iter()
                .find(|chunk| origin.matches_chunk(chunk))
        });
        Ok(Self {
            written,
            planned,
            materials: &instance.plan,
            chunk,
        })
    }

    pub(crate) fn rows(
        self,
        mesh: &'a Mesh,
    ) -> impl ExactSizeIterator<Item = MeshPrimitiveMap> + 'a {
        self.written
            .primitives
            .iter()
            .zip(&self.planned.primitives)
            .enumerate()
            .map(move |(primitive, (written, planned))| {
                let material_index = written.material.map(|index| index.value());
                let selection =
                    material_index.and_then(|_| self.materials.slot(planned.material_id));
                let material_source = match selection {
                    Some(Slot::Library(slot)) => PrimitiveMaterialSource::Library(slot),
                    Some(Slot::Default) if self.materials.has_physical_fallback() => {
                        PrimitiveMaterialSource::SuppliedFallback
                    }
                    Some(Slot::Default) => PrimitiveMaterialSource::BuiltInFallback,
                    None => PrimitiveMaterialSource::Unassigned,
                };
                MeshPrimitiveMap {
                    primitive,
                    index_accessor: written.indices.map(|index| index.value()),
                    material_index,
                    material_source,
                    model_submesh: planned.model_submesh,
                    original: self
                        .chunk
                        .and_then(|chunk| mesh.source_submesh(planned.model_submesh, chunk)),
                }
            })
    }
}
