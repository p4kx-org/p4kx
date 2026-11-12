// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_chunks::{ChunkFile, Container};

use crate::{
    CompanionResolver, LodSelection, MaterialReading, Model, ModelError, Node, PositionBox,
};
use crate::{build, crch};

/// Coordinates from the primary file of one successfully decoded model.
#[derive(Debug, Default)]
pub struct ModelNodeSources(Vec<ModelNodeSource>);

impl ModelNodeSources {
    /// Returns original fields at the decoded node ordinal.
    #[must_use]
    pub fn node(&self, ordinal: usize) -> Option<&ModelNodeSource> {
        self.0.get(ordinal)
    }

    /// Returns original fields along the selected node's parent route.
    pub fn lineage<'a>(
        &'a self,
        model: &'a Model,
        node: usize,
    ) -> impl Iterator<Item = (usize, &'a ModelNodeSource)> {
        model
            .node_ancestors(node)
            .map_while(|(ordinal, _)| Some((ordinal, self.node(ordinal)?)))
    }

    pub(crate) fn push(&mut self, node: ModelNodeSource) {
        self.0.push(node);
    }
}

/// Original byte ranges used to decode one node of a model.
#[derive(Debug)]
pub struct ModelNodeSource {
    pub(crate) chunk: usize,
    pub(crate) name: Option<Range<u64>>,
    pub(crate) parent: Range<u64>,
    pub(crate) matrix: Range<u64>,
    pub(crate) id: Option<Range<u64>>,
}

impl ModelNodeSource {
    /// Returns the physical chunk's table ordinal.
    #[must_use]
    pub const fn chunk(&self) -> usize {
        self.chunk
    }

    /// Returns the half-open name range in the primary file.
    #[must_use]
    pub fn name(&self) -> Option<Range<u64>> {
        self.name.clone()
    }

    /// Returns the half-open range of the original parent field.
    #[must_use]
    pub fn parent(&self) -> Range<u64> {
        self.parent.clone()
    }

    /// Returns the half-open range of the original local matrix.
    #[must_use]
    pub fn matrix(&self) -> Range<u64> {
        self.matrix.clone()
    }

    /// Returns the half-open range of the node's chunk ID, when stored.
    #[must_use]
    pub fn id(&self) -> Option<Range<u64>> {
        self.id.clone()
    }
}

impl Model {
    /// Returns the selected node and its parents, nearest first.
    pub fn node_ancestors(&self, node: usize) -> impl Iterator<Item = (usize, &Node)> {
        let mut at = Some(node);
        (0..self.nodes.len()).map_while(move |_| {
            let index = at?;
            let node = self.nodes.get(index)?;
            at = node.parent;
            Some((index, node))
        })
    }

    /// Reads the model with original node coordinates in its primary file.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::read_with_materials`].
    pub fn read_with_node_sources<B: AsRef<[u8]>>(
        name: &str,
        file: &ChunkFile<B>,
        companions: &mut dyn CompanionResolver,
        lods: LodSelection,
        position_box: PositionBox,
        materials: MaterialReading,
        sources: &mut ModelNodeSources,
    ) -> Result<Self, ModelError> {
        let mut captured = ModelNodeSources::default();
        let model = read(
            name,
            file,
            companions,
            lods,
            position_box,
            materials,
            Some(&mut captured),
        )?;
        *sources = captured;
        Ok(model)
    }
}

pub(crate) fn read<B: AsRef<[u8]>>(
    name: &str,
    file: &ChunkFile<B>,
    companions: &mut dyn CompanionResolver,
    lods: LodSelection,
    position_box: PositionBox,
    materials: MaterialReading,
    sources: Option<&mut ModelNodeSources>,
) -> Result<Model, ModelError> {
    match file.container() {
        Container::Ivo => build::build(
            name,
            file,
            companions,
            lods,
            materials,
            position_box,
            sources,
        ),
        Container::CrCh => crch::build(name, file, companions, lods, materials, sources),
    }
}
