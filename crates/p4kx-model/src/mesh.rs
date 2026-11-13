// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::mesh_origin::MeshSubmeshOrigin;
use crate::reader::to_usize;
use crate::skin::{RawSubmesh, SkinMesh};
use crate::smooth_normals::smooth_normals;
use crate::{CorruptChunk, Mesh, Submesh};

const UNUSED: u32 = u32::MAX;
const OPAQUE_WHITE: [u8; 4] = [u8::MAX; 4];

/// Splits a mesh chunk into one [`Mesh`] per node, each holding only the vertices its submeshes
/// use; without nodes every submesh belongs to node 0.
pub(crate) fn split_by_node(
    skin: &SkinMesh<'_>,
    nodes: Option<usize>,
) -> Result<Vec<Mesh>, CorruptChunk> {
    let mut order: Vec<(usize, usize)> = Vec::with_capacity(skin.submeshes.len());
    let mut drawn = 0u64;
    for (position, submesh) in skin.submeshes.iter().enumerate() {
        check_range(position, submesh, skin.index_count)?;
        drawn += u64::from(submesh.index_count);
        if drawn > u64::from(skin.index_count) {
            return Err(CorruptChunk::IndexTotal {
                drawn,
                indices: skin.index_count,
            });
        }
        let node = match nodes {
            Some(count) if usize::from(submesh.node) >= count => {
                return Err(CorruptChunk::NodeRange {
                    submesh: position,
                    node: submesh.node,
                    nodes: count,
                });
            }
            Some(_) => usize::from(submesh.node),
            None => 0,
        };
        if submesh.index_count > 0 {
            order.push((node, position));
        }
    }
    order.sort_by_key(|&(node, _)| node);

    let mut remap = vec![UNUSED; to_usize(skin.vertex_count)];
    let mut meshes = Vec::new();
    for group in order.chunk_by(|a, b| a.0 == b.0) {
        let Some(&(node, _)) = group.first() else {
            continue;
        };
        let mut builder = MeshBuilder::new(node, skin, group);
        for &(_, position) in group {
            if let Some(submesh) = skin.submeshes.get(position) {
                builder.add(position, submesh, &mut remap)?;
            }
        }
        meshes.push(builder.finish(&mut remap));
    }
    Ok(meshes)
}

pub(crate) fn check_range(
    position: usize,
    submesh: &RawSubmesh,
    total: u32,
) -> Result<(), CorruptChunk> {
    let fits = submesh
        .first_index
        .checked_add(submesh.index_count)
        .is_some_and(|end| end <= total);
    if !fits {
        return Err(CorruptChunk::SubmeshRange {
            submesh: position,
            first: submesh.first_index,
            count: submesh.index_count,
            total,
        });
    }
    if !submesh.index_count.is_multiple_of(3) {
        return Err(CorruptChunk::NotTriangles {
            submesh: position,
            count: submesh.index_count,
        });
    }
    Ok(())
}

struct MeshBuilder<'s, 'a> {
    skin: &'s SkinMesh<'a>,
    mesh: Mesh,
    used: Vec<u32>,
}

impl<'s, 'a> MeshBuilder<'s, 'a> {
    fn new(node: usize, skin: &'s SkinMesh<'a>, group: &[(usize, usize)]) -> Self {
        let indices: usize = group
            .iter()
            .filter_map(|&(_, position)| skin.submeshes.get(position))
            .map(|submesh| to_usize(submesh.index_count))
            .sum();
        Self {
            skin,
            mesh: Mesh {
                node,
                positions: Vec::new(),
                source: Vec::new(),
                origin: None,
                index_width: Some(skin.streams.indices.width()),
                normals: skin.streams.normals.map(|_| Vec::new()),
                normals_computed: false,
                tangents: skin.streams.tangents.map(|_| Vec::new()),
                colours: Some(Vec::new()),
                influences: skin.streams.bone_map.map(|_| Vec::new()),
                uvs: Vec::new(),
                second_uvs: skin.streams.second_uvs.map(|_| Vec::new()),
                non_finite_uvs: 0,
                indices: Vec::with_capacity(indices),
                submeshes: Vec::with_capacity(group.len()),
            },
            used: Vec::new(),
        }
    }

    fn add(
        &mut self,
        position: usize,
        submesh: &RawSubmesh,
        remap: &mut [u32],
    ) -> Result<(), CorruptChunk> {
        let first = self.mesh.indices.len();
        let start = to_usize(submesh.first_index);
        for at in start..start + to_usize(submesh.index_count) {
            let index = self
                .skin
                .streams
                .indices
                .source_vertex(at, submesh.page_base)?;
            let out_of_range = CorruptChunk::IndexRange {
                submesh: position,
                index,
                vertices: self.skin.vertex_count,
            };
            let vertex = usize::try_from(index).map_err(|_| out_of_range.clone())?;
            let slot = remap.get_mut(vertex).ok_or(out_of_range)?;
            if *slot == UNUSED {
                *slot = self.push_vertex(vertex)?;
                self.used.push(u32::try_from(vertex).unwrap_or(UNUSED));
            }
            self.mesh.indices.push(*slot);
        }
        self.mesh.submeshes.push(Submesh {
            material_id: submesh.material_id,
            node_id: submesh.node,
            first_index: first,
            index_count: self.mesh.indices.len() - first,
            origin: Some(MeshSubmeshOrigin::new(
                submesh.start,
                self.skin.streams.indices.word_start(submesh.first_index),
                submesh.page_base,
            )),
        });
        Ok(())
    }

    fn push_vertex(&mut self, vertex: usize) -> Result<u32, CorruptChunk> {
        let local = u32::try_from(self.mesh.positions.len()).unwrap_or(UNUSED);
        let (position, uv, mut repaired) = self.skin.streams.vertices.vertex(vertex)?;
        self.mesh.positions.push(position);
        self.mesh
            .source
            .push(u32::try_from(vertex).unwrap_or(UNUSED));
        self.mesh.uvs.push(uv);
        if let (Some(stream), Some(normals)) = (&self.skin.streams.normals, &mut self.mesh.normals)
        {
            normals.push(stream.normal(vertex)?);
        }
        if let (Some(stream), Some(tangents)) =
            (&self.skin.streams.tangents, &mut self.mesh.tangents)
            && let Some(tangent) = stream.tangent(vertex)?
        {
            tangents.push(tangent);
        }
        if let (Some(stream), Some(second_uvs)) =
            (&self.skin.streams.second_uvs, &mut self.mesh.second_uvs)
        {
            let (second, second_repaired) = stream.uv(vertex)?;
            second_uvs.push(second);
            repaired |= second_repaired;
        }
        self.mesh.non_finite_uvs += usize::from(repaired);
        if let Some(colours) = &mut self.mesh.colours {
            colours.push(self.skin.streams.vertices.colour(vertex)?);
        }
        if let (Some(stream), Some(influences)) =
            (&self.skin.streams.bone_map, &mut self.mesh.influences)
        {
            influences.push(stream.influences(vertex)?);
        }
        Ok(local)
    }

    fn finish(mut self, remap: &mut [u32]) -> Mesh {
        if self.mesh.normals.is_none() && !self.mesh.indices.is_empty() {
            self.mesh.normals = Some(smooth_normals(&self.mesh.positions, &self.mesh.indices));
            self.mesh.normals_computed = true;
        }
        if self
            .mesh
            .colours
            .as_ref()
            .is_some_and(|colours| colours.iter().all(|&colour| colour == OPAQUE_WHITE))
        {
            self.mesh.colours = None;
        }
        for vertex in self.used {
            if let Some(slot) = remap.get_mut(to_usize(vertex)) {
                *slot = UNUSED;
            }
        }
        self.mesh
    }
}
