// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};

use gltf_json::Root;
use gltf_json::mesh::Semantic;
use gltf_json::validation::Checked::Valid;
use p4kx_model::{Mesh, MeshOrigin, MeshVertexOrigin};

use crate::instance::Instance;
use crate::layout::Layout;
use crate::mesh_primitive_map;
use crate::mesh_primitive_map::MeshPrimitiveMap;
use crate::scene::{Scene, write_scene};
use crate::texture_loader::{NoTextures, TextureLoader};
use crate::{GlbOptions, GlbSummary, GltfError};

/// The actual vertex order of one emitted GLB mesh.
#[derive(Debug, Clone, Copy)]
#[non_exhaustive]
pub struct MeshVertexMap<'a> {
    /// The GLB mesh index.
    pub mesh: usize,
    /// The GLB POSITION accessor shared by this mesh's primitives.
    pub position_accessor: usize,
    /// The count of actually written POSITION elements.
    pub vertices: u64,
    /// The model instance that supplied the geometry, in producer order.
    pub part: usize,
    /// The scene node whose model produced this mesh, when writing a scene.
    pub scene_node: Option<usize>,
    /// The originating node index in that instance's model.
    pub model_node: usize,
    /// The requested GLB level.
    pub requested_lod: u32,
    /// The selected level in the decoded model.
    pub model_lod: u32,
    /// The original IVO chunk and geometry level, when its source is known.
    pub origin: Option<MeshOrigin>,
    /// One physical stream vertex ordinal per emitted POSITION element when proven.
    pub source_vertices: Option<&'a [u32]>,
    mesh_source: &'a Mesh,
    primitives: mesh_primitive_map::ObservedMesh<'a>,
}

impl MeshVertexMap<'_> {
    /// Streams the output primitives and their actual source submesh associations.
    pub fn primitives(&self) -> impl ExactSizeIterator<Item = MeshPrimitiveMap> + '_ {
        self.primitives.rows(self.mesh_source)
    }

    /// Returns original first-use index coordinates when the position order is known.
    pub fn vertex_origins(&self) -> impl Iterator<Item = MeshVertexOrigin> + '_ {
        self.source_vertices
            .into_iter()
            .flat_map(|_| self.mesh_source.vertex_origins())
    }
}

/// Writes a scene and visits each emitted GLB mesh and its source vertex map.
///
/// Shared meshes appear once, owned by their producing model instance.
///
/// # Errors
/// Returns the same errors as the ordinary writer, or a write error from the visitor.
pub fn write_scene_glb_observed(
    scene: &Scene<'_>,
    options: &GlbOptions,
    textures: Option<&dyn TextureLoader>,
    out: &mut impl Write,
    visitor: &mut impl FnMut(MeshVertexMap<'_>) -> io::Result<()>,
) -> Result<GlbSummary, GltfError> {
    write_scene(
        scene,
        options,
        textures.unwrap_or(&NoTextures),
        out,
        Some(visitor),
    )
}

pub(crate) type ObserveMesh<'a> = dyn FnMut(MeshVertexMap<'_>) -> io::Result<()> + 'a;

pub(crate) fn observe(
    document: &Root,
    layout: &Layout<'_>,
    instances: &[Instance<'_>],
    requested_lod: u32,
    visitor: &mut ObserveMesh<'_>,
) -> io::Result<()> {
    if document.meshes.len() != layout.meshes.len() {
        return Err(layout_error());
    }
    for (index, (written, plan)) in document.meshes.iter().zip(&layout.meshes).enumerate() {
        let instance = instances.get(plan.part).ok_or_else(layout_error)?;
        let lod = instance.lod.ok_or_else(layout_error)?;
        let source = lod.meshes.get(plan.model_mesh).ok_or_else(layout_error)?;
        let position = written
            .primitives
            .first()
            .and_then(|primitive| primitive.attributes.get(&Valid(Semantic::Positions)))
            .copied()
            .ok_or_else(layout_error)?;
        let accessor = document
            .accessors
            .get(position.value())
            .ok_or_else(layout_error)?;
        let origin = source.origin();
        let primitives =
            mesh_primitive_map::ObservedMesh::new(document, written, plan, instance, source)?;
        visitor(MeshVertexMap {
            mesh: index,
            position_accessor: position.value(),
            vertices: accessor.count.0,
            part: plan.part,
            scene_node: instance.wrapper,
            model_node: source.node,
            requested_lod,
            model_lod: lod.level,
            origin,
            mesh_source: source,
            primitives,
            source_vertices: (source.has_vertex_origin_order()
                && source.source.len() as u64 == accessor.count.0)
                .then_some(source.source.as_slice()),
        })?;
    }
    Ok(())
}

fn layout_error() -> io::Error {
    io::Error::other("GLB mesh and source layout disagree")
}
