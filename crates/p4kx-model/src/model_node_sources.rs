// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_chunks::{ChunkFile, Container};

use crate::{
    CompanionResolver, LodSelection, MaterialReading, Model, ModelBoneSources, ModelError,
    ModelNodeSelection, ModelNodeSource, Node, PositionBox, PrimarySkeletonSelection,
};
use crate::{build, crch};

/// Original node and skeleton coordinates for one model read.
#[derive(Debug, Default)]
#[non_exhaustive]
pub struct ModelReadSources {
    /// Node fields of the primary model.
    pub nodes: ModelNodeSources,
    /// Skeleton fields keyed by the file actually read.
    pub skeletons: Vec<ModelBoneSources>,
    /// The survey of the primary file's skeleton selection.
    pub primary_skeleton: Option<PrimarySkeletonSelection>,
}

#[derive(Default)]
pub(crate) struct ReadCapture<'a> {
    pub(crate) nodes: Option<&'a mut ModelNodeSources>,
    pub(crate) skeletons: Option<&'a mut Vec<ModelBoneSources>>,
    pub(crate) primary_skeleton: Option<&'a mut Option<PrimarySkeletonSelection>>,
}

/// Coordinates from the primary file of one successfully decoded model.
#[derive(Debug, Default)]
pub struct ModelNodeSources {
    nodes: Vec<ModelNodeSource>,
    selection: Option<ModelNodeSelection>,
}

impl ModelNodeSources {
    /// Returns original fields at the decoded node ordinal.
    #[must_use]
    pub fn node(&self, ordinal: usize) -> Option<&ModelNodeSource> {
        self.nodes.get(ordinal)
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

    /// Returns the original primary node lookup.
    #[must_use]
    pub const fn selection(&self) -> Option<&ModelNodeSelection> {
        self.selection.as_ref()
    }

    /// Returns whether the observed decoder synthesized its fallback root.
    #[must_use]
    pub const fn uses_synthetic_root(&self) -> bool {
        self.selection.is_some() && self.nodes.is_empty()
    }

    pub(crate) const fn set_selection(&mut self, selection: ModelNodeSelection) {
        self.selection = Some(selection);
    }

    pub(crate) fn push(&mut self, node: ModelNodeSource) {
        self.nodes.push(node);
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
            ReadCapture {
                nodes: Some(&mut captured),
                skeletons: None,
                primary_skeleton: None,
            },
        )?;
        *sources = captured;
        Ok(model)
    }

    /// Reads the model with original node and skeleton coordinates.
    ///
    /// # Errors
    ///
    /// Returns the same errors as [`Self::read_with_materials`].
    pub fn read_with_sources<B: AsRef<[u8]>>(
        name: &str,
        file: &ChunkFile<B>,
        companions: &mut dyn CompanionResolver,
        lods: LodSelection,
        position_box: PositionBox,
        materials: MaterialReading,
        sources: &mut ModelReadSources,
    ) -> Result<Self, ModelError> {
        let mut captured = ModelReadSources::default();
        let model = read(
            name,
            file,
            companions,
            lods,
            position_box,
            materials,
            ReadCapture {
                nodes: Some(&mut captured.nodes),
                skeletons: Some(&mut captured.skeletons),
                primary_skeleton: Some(&mut captured.primary_skeleton),
            },
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
    sources: ReadCapture<'_>,
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
        Container::CrCh => {
            if let Some(selection) = sources.primary_skeleton {
                *selection = Some(PrimarySkeletonSelection::ContainerOnly);
            }
            crch::build(name, file, companions, lods, materials, sources.nodes)
        }
    }
}
