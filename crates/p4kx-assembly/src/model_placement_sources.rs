// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::num::NonZeroUsize;

use p4kx_extraction::{EntryOrigin, SourceLocation};
use p4kx_model::{Model, ModelFile, ModelNodeSource, ModelNodeSources};

use crate::placement::BoneSelection;
use crate::{Assembly, Part};

#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct ModelPlacement {
    pub(crate) anchor: ModelAnchor,
    offset: Option<NonZeroUsize>,
}

impl ModelPlacement {
    pub(crate) fn new(anchor: ModelAnchor, offset: Option<usize>) -> Self {
        Self {
            anchor,
            offset: offset.and_then(index),
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub(crate) enum ModelAnchor {
    #[default]
    None,
    Node(usize),
    Bone {
        owner: BoneOwner,
        selection: BoneSelection,
    },
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum BoneOwner {
    Parent,
    Root,
}

fn index(ordinal: usize) -> Option<NonZeroUsize> {
    ordinal.checked_add(1).and_then(NonZeroUsize::new)
}

/// Original model inputs for one consumed node placement.
#[derive(Debug)]
pub struct ModelPlacementInput<'a> {
    owner: &'a Part,
    model: &'a Model,
    origin: &'a EntryOrigin,
    sources: &'a ModelNodeSources,
    selected: usize,
}

impl<'a> ModelPlacementInput<'a> {
    fn new(part: &'a Part, selected: NonZeroUsize) -> Option<Self> {
        let primary = part
            .model_archive_sources()
            .iter()
            .find(|source| source.file() == ModelFile::Model)?;
        let sources = primary.node_sources()?;
        let selected = selected.get() - 1;
        sources.node(selected)?;
        Some(Self {
            owner: part,
            model: part.model()?,
            origin: primary.origin()?,
            sources,
            selected,
        })
    }

    /// Returns the actual primary Archive Entry of the model supplying these nodes.
    #[must_use]
    pub const fn origin(&self) -> &'a EntryOrigin {
        self.origin
    }

    /// Returns the authored selector for the model supplying this placement.
    #[must_use]
    pub fn geometry_selector(&self) -> Option<SourceLocation> {
        self.owner.geometry_selector()
    }

    /// Returns the physical character definition that selected this model.
    #[must_use]
    pub const fn definition_origin(&self) -> Option<&EntryOrigin> {
        self.owner.model_definition_origin()
    }

    /// Returns source coordinates along the transform's actual selected-node-to-root route.
    pub fn nodes(&self) -> impl Iterator<Item = (usize, &'a ModelNodeSource)> {
        self.sources.lineage(self.model, self.selected)
    }
}

impl Assembly {
    /// Returns the observed original model nodes used by the part's local placement.
    pub fn model_placement_inputs(
        &self,
        part: usize,
    ) -> impl Iterator<Item = ModelPlacementInput<'_>> {
        self.parts.get(part).into_iter().flat_map(|part| {
            let nodes = part.model_placement;
            let anchor = if let ModelAnchor::Node(node) = nodes.anchor {
                part.parent()
                    .and_then(|parent| self.parts.get(parent))
                    .and_then(|parent| ModelPlacementInput::new(parent, index(node)?))
            } else {
                None
            };
            let offset = nodes
                .offset
                .and_then(|node| ModelPlacementInput::new(part, node));
            [anchor, offset].into_iter().flatten()
        })
    }
}
