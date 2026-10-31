// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::Write;

use p4kx_model::{Influences, Lod, MAX_INFLUENCES, Mesh};
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use crate::axes::to_y_up;
use crate::block_data::BlockData;
use crate::colour_plan::Colours;
use crate::materials::MaterialPlan;
use crate::morphs::{MorphLayout, MorphTarget};
use crate::skin::SET_LEN;
use crate::texture_plan::EncodedImage;

const ALIGNMENT: u64 = 4;
const BATCH: usize = 1 << 14;
const WINDOW: u64 = 32 << 20;

/// One block of the binary chunk at its offset.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Block<'m> {
    pub(crate) offset: u64,
    pub(crate) len: u64,
    pub(crate) data: BlockData<'m>,
}

/// The primitive of one submesh: its index block and material ID.
#[derive(Debug, Clone, Copy)]
pub(crate) struct PrimitiveLayout {
    pub(crate) indices: usize,
    pub(crate) material_id: u16,
    /// Whether the primitive carries the mesh's vertex colours as `COLOR_0`.
    pub(crate) colours: bool,
}

/// The blocks of one mesh and the bounds of its converted positions.
#[derive(Debug, Clone)]
pub(crate) struct MeshLayout {
    pub(crate) part: usize,
    pub(crate) node: usize,
    pub(crate) positions: usize,
    pub(crate) normals: Option<usize>,
    pub(crate) uvs: Option<usize>,
    pub(crate) tangents: Option<usize>,
    pub(crate) colours: Option<usize>,
    pub(crate) joints: Vec<usize>,
    pub(crate) weights: Vec<usize>,
    pub(crate) primitives: Vec<PrimitiveLayout>,
    pub(crate) morphs: Vec<MorphLayout>,
    pub(crate) min: [f32; 3],
    pub(crate) max: [f32; 3],
}

/// The skeleton a part is skinned to: its inverse bind matrices, and per mesh the influences to
/// write instead of the mesh's own.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Rigging<'m> {
    pub(crate) binds: Option<&'m [[f32; 16]]>,
    pub(crate) influences: Option<&'m [Vec<Influences>]>,
    pub(crate) morphs: &'m [MorphTarget],
}

/// The inverse bind matrices of a mesh's skeleton and the influences to write for the mesh.
type SkinSource<'m> = (Option<&'m [[f32; 16]]>, Option<&'m [Influences]>);

/// The place of one embedded image in the binary chunk.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ImageBlock {
    pub(crate) offset: u64,
    pub(crate) len: u64,
}

/// The binary chunk planned before anything is written: every block with its offset.
#[derive(Debug, Clone)]
pub(crate) struct Layout<'m> {
    pub(crate) blocks: Vec<Block<'m>>,
    pub(crate) meshes: Vec<MeshLayout>,
    pub(crate) images: Vec<ImageBlock>,
    pub(crate) inverse_binds: Option<usize>,
    pub(crate) len: u64,
    pub(crate) hidden_primitives: usize,
    pub(crate) max_influences: usize,
}

impl<'m> Layout<'m> {
    /// Creates a layout without blocks.
    pub(crate) const fn new() -> Self {
        Self {
            blocks: Vec::new(),
            meshes: Vec::new(),
            images: Vec::new(),
            inverse_binds: None,
            len: 0,
            hidden_primitives: 0,
            max_influences: MAX_INFLUENCES,
        }
    }

    /// Returns the layout writing at most `count` bones per vertex.
    pub(crate) const fn limited(mut self, count: usize) -> Self {
        self.max_influences = count;
        self
    }

    /// Plans the blocks of every mesh of `lod`, a level of the model instance `part`, that has
    /// vertices and triangles, leaving out the submeshes whose material draws nothing; vertex
    /// colours as `colours` says. A skinned mesh gets its bone weights when the
    /// inverse bind matrices of `rigging` are given; its influences, when it has any, replace
    /// those of the meshes.
    pub(crate) fn add_part(
        &mut self,
        part: usize,
        lod: &'m Lod,
        colours: Colours,
        materials: &MaterialPlan<'_>,
        rigging: Rigging<'m>,
    ) {
        for (position, mesh) in lod.meshes.iter().enumerate() {
            let own = rigging
                .influences
                .and_then(|all| all.get(position))
                .map(Vec::as_slice);
            let before = self.meshes.len();
            self.plan_mesh(part, mesh, colours, materials, (rigging.binds, own));
            if position == 0 && self.meshes.len() > before {
                self.plan_morphs(mesh, rigging.morphs);
            }
        }
    }

    fn plan_skin(
        &mut self,
        mesh: &'m Mesh,
        binds: Option<&'m [[f32; 16]]>,
        remapped: Option<&'m [Influences]>,
    ) -> (Vec<usize>, Vec<usize>) {
        let influences = remapped
            .or(mesh.influences.as_deref())
            .filter(|influences| influences.len() == mesh.positions.len());
        let (Some(influences), Some(binds)) = (influences, binds) else {
            return (Vec::new(), Vec::new());
        };
        if self.inverse_binds.is_none() {
            self.inverse_binds = Some(self.push(BlockData::InverseBinds(binds)));
        }
        let limit = self.max_influences;
        let sets = if influences
            .iter()
            .any(|influences| influences.count().min(limit) > SET_LEN)
        {
            2
        } else {
            1
        };
        let mut joints = Vec::new();
        let mut weights = Vec::new();
        for set in 0..sets {
            joints.push(self.push(BlockData::Joints(influences, (set, limit))));
            weights.push(self.push(BlockData::Weights(influences, (set, limit))));
        }
        (joints, weights)
    }

    fn plan_mesh(
        &mut self,
        part: usize,
        mesh: &'m Mesh,
        write: Colours,
        materials: &MaterialPlan<'_>,
        skin: SkinSource<'m>,
    ) {
        let (binds, remapped) = skin;
        let drawn: Vec<(&'m [u32], u16)> = mesh
            .submeshes
            .iter()
            .filter_map(|submesh| {
                let end = submesh.first_index.checked_add(submesh.index_count)?;
                let indices = mesh.indices.get(submesh.first_index..end)?;
                (!indices.is_empty()).then_some((indices, submesh.material_id))
            })
            .collect();
        let primitives: Vec<(&'m [u32], u16)> = drawn
            .iter()
            .copied()
            .filter(|&(_, id)| !materials.hidden(id))
            .collect();
        self.hidden_primitives += drawn.len() - primitives.len();
        if mesh.positions.is_empty() || primitives.is_empty() {
            return;
        }
        let (min, max) = bounds(&mesh.positions);
        let positions = self.push(BlockData::Positions(&mesh.positions));
        let normals = mesh
            .normals
            .as_ref()
            .filter(|normals| normals.len() == mesh.positions.len())
            .map(|normals| self.push(BlockData::Normals(normals)));
        let uvs =
            (mesh.uvs.len() == mesh.positions.len()).then(|| self.push(BlockData::Uvs(&mesh.uvs)));
        let tangents = mesh
            .tangents
            .as_ref()
            .filter(|tangents| tangents.len() == mesh.positions.len())
            .map(|tangents| self.push(BlockData::Tangents(tangents)));
        let reading: Vec<bool> = primitives
            .iter()
            .map(|&(_, id)| materials.writes_vertex_colours(id, write))
            .collect();
        let colours = mesh
            .colours
            .as_ref()
            .filter(|colours| colours.len() == mesh.positions.len())
            .filter(|_| reading.contains(&true))
            .map(|colours| self.push(BlockData::Colours(colours)));
        let (joints, weights) = self.plan_skin(mesh, binds, remapped);
        let primitives = self.plan_primitives(&primitives, &reading, mesh.positions.len());
        self.meshes.push(MeshLayout {
            part,
            node: mesh.node,
            positions,
            normals,
            uvs,
            tangents,
            colours,
            joints,
            weights,
            primitives,
            morphs: Vec::new(),
            min,
            max,
        });
    }

    fn plan_primitives(
        &mut self,
        primitives: &[(&'m [u32], u16)],
        reading: &[bool],
        vertices: usize,
    ) -> Vec<PrimitiveLayout> {
        let short = u16::try_from(vertices).is_ok();
        primitives
            .iter()
            .zip(reading)
            .map(|(&(indices, material_id), &colours)| PrimitiveLayout {
                indices: self.push(if short {
                    BlockData::Short(indices)
                } else {
                    BlockData::Wide(indices)
                }),
                material_id,
                colours,
            })
            .collect()
    }

    pub(crate) fn push(&mut self, data: BlockData<'m>) -> usize {
        let len = data.len();
        self.blocks.push(Block {
            offset: self.len,
            len,
            data,
        });
        self.len = self.len.saturating_add(len.next_multiple_of(ALIGNMENT));
        self.blocks.len() - 1
    }

    /// Returns the material IDs of the primitives of the model instance `part`, sorted, each once.
    pub(crate) fn part_material_ids(&self, part: usize) -> Vec<u16> {
        self.ids_where(|mesh| mesh.part == part)
    }

    fn ids_where(&self, keep: impl Fn(&MeshLayout) -> bool) -> Vec<u16> {
        let mut ids: Vec<u16> = self
            .meshes
            .iter()
            .filter(|mesh| keep(mesh))
            .flat_map(|mesh| &mesh.primitives)
            .map(|primitive| primitive.material_id)
            .collect();
        ids.sort_unstable();
        ids.dedup();
        ids
    }

    /// Plans an image block for each of the `lens` byte lengths, after every other block.
    pub(crate) fn push_images(&mut self, lens: impl IntoIterator<Item = u64>) {
        for len in lens {
            self.images.push(ImageBlock {
                offset: self.len,
                len,
            });
            self.len = self.len.saturating_add(len.next_multiple_of(ALIGNMENT));
        }
    }

    /// Writes every block with its padding to `out`, then the PNG bytes of `images` in the order
    /// of their blocks. Runs of small blocks are encoded in parallel, a window of at most
    /// [`WINDOW`] bytes at a time; a block alone in its window is streamed a batch of elements at
    /// a time through one reused buffer.
    pub(crate) fn write(
        &self,
        out: &mut impl Write,
        images: &[EncodedImage],
    ) -> std::io::Result<()> {
        let mut start = 0;
        while start < self.blocks.len() {
            let mut end = start;
            let mut bytes = 0_u64;
            while let Some(block) = self.blocks.get(end) {
                if end > start && bytes.saturating_add(block.len) > WINDOW {
                    break;
                }
                bytes = bytes.saturating_add(block.len);
                end += 1;
            }
            match self.blocks.get(start..end).unwrap_or_default() {
                [block] => block.stream(out)?,
                window => {
                    let encoded: Vec<Vec<u8>> = window.par_iter().map(Block::encoded).collect();
                    for bytes in &encoded {
                        out.write_all(bytes)?;
                    }
                }
            }
            start = end;
        }
        for (image, block) in images.iter().zip(&self.images) {
            out.write_all(&image.png)?;
            let padding = block.len.next_multiple_of(ALIGNMENT) - block.len;
            out.write_all(
                [0; 4]
                    .get(..usize::try_from(padding).unwrap_or(0))
                    .unwrap_or_default(),
            )?;
        }
        Ok(())
    }
}

impl Block<'_> {
    fn padding(&self) -> usize {
        usize::try_from(self.len.next_multiple_of(ALIGNMENT) - self.len).unwrap_or(0)
    }

    fn encoded(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(usize::try_from(self.len).unwrap_or(0) + 4);
        self.data.encode(&mut bytes);
        bytes.resize(bytes.len() + self.padding(), 0);
        bytes
    }

    fn stream(&self, out: &mut impl Write) -> std::io::Result<()> {
        let mut buffer = Vec::new();
        let count = self.data.count();
        for start in (0..count).step_by(BATCH) {
            buffer.clear();
            self.data
                .slice(start, (start + BATCH).min(count))
                .encode(&mut buffer);
            out.write_all(&buffer)?;
        }
        out.write_all([0; 4].get(..self.padding()).unwrap_or_default())
    }
}

fn bounds(positions: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::INFINITY; 3];
    let mut max = [f32::NEG_INFINITY; 3];
    for &position in positions {
        for ((low, high), value) in min.iter_mut().zip(&mut max).zip(to_y_up(position)) {
            *low = low.min(value);
            *high = high.max(value);
        }
    }
    (min, max)
}
