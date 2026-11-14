// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::Range;

use p4kx_extraction::{
    DataForge, DataForgeEntry, EntityMaterialEffects, EntityMaterialEffectsError, FieldOrigins,
    FieldPath, FieldPathError, ModelArchiveSource,
};
use p4kx_model::{MaterialStatus, Model, ModelFile};
use thiserror::Error;

use crate::{Assembly, Part};

#[path = "entity_glow_links.rs"]
mod links;

#[cfg(test)]
#[path = "entity_glow_tests.rs"]
mod tests;

pub use links::{EntityGlowLink, GlowEffectKind};

/// Source declarations and geometry associations for one rendered Entity part.
#[derive(Debug)]
#[non_exhaustive]
pub struct EntityGlow<'a> {
    /// The declaring Entity's original material effects and individual field origins.
    pub authored: EntityMaterialEffects,
    /// Each separate authored Glow link in component, effect and link order.
    pub links: Vec<EntityGlowLink>,
    /// Original archive model and companion reads behind geometry coordinates.
    pub model_sources: &'a [ModelArchiveSource],
}

/// An Entity owner, declaration or selector could not be read.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum EntityGlowBindingError {
    /// The supplied input differs from the source of the assembly.
    #[error("the DataForge input does not own the assembly")]
    SourceIdentity,
    /// The owner is an invalid or synthetic part.
    #[error("assembly part {0} has no original Entity record")]
    MissingEntity(usize),
    /// Exact Entity metadata or its authored declarations could not be loaded.
    #[error(transparent)]
    Read(#[from] EntityMaterialEffectsError),
    /// The exact canonical source pointer cannot be represented.
    #[error(transparent)]
    Pointer(#[from] FieldPathError),
}

impl Part {
    /// Returns the original DataForge Record position of an Entity-backed part.
    #[must_use]
    pub const fn entity_record_index(&self) -> Option<u32> {
        self.entity_record_index
    }
}

impl Assembly {
    /// Resolves Glow declarations against the selected owner's authored model nodes and subsets.
    ///
    /// # Errors
    /// Returns an owner identity or source read error.
    pub fn entity_glow(
        &self,
        part: usize,
        entry: &DataForgeEntry,
        forge: &DataForge<'_>,
    ) -> Result<EntityGlow<'_>, EntityGlowBindingError> {
        let owner = self
            .parts()
            .get(part)
            .ok_or(EntityGlowBindingError::MissingEntity(part))?;
        let record = owner
            .entity_record_index()
            .ok_or(EntityGlowBindingError::MissingEntity(part))?;
        let origin = entry.origin_of(forge).filter(|origin| {
            !matches!(origin, p4kx_extraction::DataForgeOrigin::Unknown)
                && self.forge_origin() == Some(origin)
        });
        if origin.is_none() {
            return Err(EntityGlowBindingError::SourceIdentity);
        }
        let authored = EntityMaterialEffects::read(entry, forge, record, owner.guid())?;
        let model_sources = owner.model_archive_sources();
        let binding = Binding::of(owner.model(), owner.materials(), model_sources);
        Ok(EntityGlow {
            links: links::read(&authored, &binding)?,
            authored,
            model_sources,
        })
    }
}

/// Result of applying an authored Glow selector to its owning Entity's geometry.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum GlowSelection {
    /// No selector field was stored.
    Absent,
    /// The selector field does not hold text.
    InvalidSelector,
    /// No decoded original model is available for this owner.
    MissingModel,
    /// The selector cannot be joined to original model node names.
    UnobservedNodes,
    /// No original node matches this selector.
    Unmatched,
    /// Several original nodes match the name and the link remains unbound.
    Ambiguous {
        /// The model's original node ordinals in source order.
        nodes: Vec<usize>,
    },
    /// Matching original nodes exist but own no drawable subsets.
    NoDrawableSubsets,
    /// The selector reached these original model subsets.
    Bound(Vec<GlowSubset>),
}

/// One geometry subset of a named original model node.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct GlowSubset {
    /// Decoded LOD label used by the selected assembly.
    pub lod: u32,
    /// Model node ordinal in original model order.
    pub node: usize,
    /// Physical original node chunk ordinal and half-open name range.
    pub node_name_source: Option<(usize, Range<u64>)>,
    /// Original node ID carried by this subset.
    pub source_node_id: u16,
    /// Subset ordinal in the node's mesh.
    pub subset: usize,
    /// Node-local triangle index range in the decoded model.
    pub indices: Range<usize>,
    /// Original material slot and material resolution from the actual selected library.
    pub material: GlowSubsetMaterial,
}

/// Material selection of a drawable original subset.
#[derive(Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct GlowSubsetMaterial {
    /// Original material ID carried by the subset.
    pub source_slot: u16,
    /// Indicates whether the chosen material library resolves the source slot explicitly.
    pub resolves: bool,
    /// True when the resolved material suppresses drawing.
    pub no_draw: Option<bool>,
    /// Exact selected shader name when a material is present.
    pub shader: Option<String>,
}

struct Binding<'a> {
    model: Option<&'a Model>,
    materials: &'a MaterialStatus,
    model_sources: &'a [ModelArchiveSource],
}

impl<'a> Binding<'a> {
    const fn of(
        model: Option<&'a Model>,
        materials: &'a MaterialStatus,
        model_sources: &'a [ModelArchiveSource],
    ) -> Self {
        Self {
            model,
            materials,
            model_sources,
        }
    }

    fn select(&self, selector: Option<&str>) -> GlowSelection {
        let Some(selector) = selector else {
            return GlowSelection::Absent;
        };
        let Some(model) = self.model else {
            return GlowSelection::MissingModel;
        };
        let observed = self
            .model_sources
            .iter()
            .find(|source| source.file() == ModelFile::Model && source.origin().is_some())
            .and_then(ModelArchiveSource::node_sources);
        let nodes = match target_nodes(selector, model, observed) {
            Ok(nodes) => nodes,
            Err(selection) => return selection,
        };
        let subsets = self.subsets(model, &nodes, observed);
        if subsets.is_empty() {
            GlowSelection::NoDrawableSubsets
        } else {
            GlowSelection::Bound(subsets)
        }
    }

    fn subsets(
        &self,
        model: &Model,
        nodes: &TargetNodes,
        observed: Option<&p4kx_model::ModelNodeSources>,
    ) -> Vec<GlowSubset> {
        let mut result = Vec::new();
        for lod in &model.lods {
            for mesh in &lod.meshes {
                if !nodes.contains(mesh.node) {
                    continue;
                }
                for (subset, row) in mesh.submeshes.iter().enumerate() {
                    if mesh.submesh_indices(row).is_none() {
                        continue;
                    }
                    let observed = observed.and_then(|input| input.node(mesh.node));
                    let node_name_source =
                        observed.and_then(|input| Some((input.chunk(), input.name()?)));
                    let Some(end) = row.first_index.checked_add(row.index_count) else {
                        continue;
                    };
                    let material = self.material(row.material_id);
                    result.push(GlowSubset {
                        lod: lod.level,
                        node: mesh.node,
                        node_name_source,
                        source_node_id: row.node_id,
                        subset,
                        indices: row.first_index..end,
                        material,
                    });
                }
            }
        }
        result
    }

    fn material(&self, slot: u16) -> GlowSubsetMaterial {
        let authored = self
            .materials
            .library()
            .filter(|library| !library.is_empty_slot(slot))
            .and_then(|library| library.material(slot));
        GlowSubsetMaterial {
            source_slot: slot,
            resolves: authored.is_some(),
            no_draw: authored.map(|material| {
                material.flags.no_draw() || material.shader.eq_ignore_ascii_case("NoDraw")
            }),
            shader: authored.map(|material| material.shader.clone()),
        }
    }
}

enum TargetNodes {
    All,
    Name(usize),
}

impl TargetNodes {
    const fn contains(&self, node: usize) -> bool {
        match self {
            Self::All => true,
            Self::Name(owner) => node == *owner,
        }
    }
}

fn target_nodes(
    selector: &str,
    model: &Model,
    observed: Option<&p4kx_model::ModelNodeSources>,
) -> Result<TargetNodes, GlowSelection> {
    if matches!(selector, "" | "*") {
        return Ok(TargetNodes::All);
    }
    let observed = observed.ok_or(GlowSelection::UnobservedNodes)?;
    let mut nodes = Vec::new();
    for (ordinal, node) in model.nodes.iter().enumerate() {
        observed
            .node(ordinal)
            .and_then(p4kx_model::ModelNodeSource::name)
            .ok_or(GlowSelection::UnobservedNodes)?;
        if node.name.eq_ignore_ascii_case(selector) {
            nodes.push(ordinal);
        }
    }
    match nodes.as_slice() {
        [] => Err(GlowSelection::Unmatched),
        [node] => Ok(TargetNodes::Name(*node)),
        _ => Err(GlowSelection::Ambiguous { nodes }),
    }
}
