// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    ArchiveModels, Entities, EntityKind, EntityNode, LoadoutFiles, LoadoutResolver, ModelProvider,
};
use p4kx_extraction::model::Lod;
use p4kx_extraction::{Archive, Record, RecordTree};

use super::{Case, DockedVehicles, SourceCheck, observer};

#[derive(Debug, Clone, Copy)]
pub(super) enum LodEvidence {
    NotRequested,
    Exact,
    NearestLower,
    Lowest,
}

pub(super) struct Capabilities {
    kind: EntityKind,
    item_size: Option<u32>,
    requested_lod: Option<u32>,
    actual_lods: Vec<u32>,
    root_vertices: Option<usize>,
    containers: usize,
    audio_triggers: usize,
}

/// Geometry and verified source capabilities retained by a selected fixture.
pub(crate) struct Prepared {
    node: Option<EntityNode>,
    capabilities: Capabilities,
}

impl Prepared {
    pub(crate) fn node(&self) -> super::Result<&EntityNode> {
        self.node
            .as_ref()
            .ok_or_else(|| SourceCheck::ModelPresence.into())
    }

    pub(super) const fn facts(&self) -> &Capabilities {
        &self.capabilities
    }
}

impl Capabilities {
    pub(super) fn lod_evidence(&self) -> LodEvidence {
        match self.requested_lod {
            None => LodEvidence::NotRequested,
            Some(requested) if self.actual_lods.contains(&requested) => LodEvidence::Exact,
            Some(requested) if self.actual_lods.iter().any(|actual| *actual < requested) => {
                LodEvidence::NearestLower
            }
            Some(_) => LodEvidence::Lowest,
        }
    }

    fn satisfies(&self, case: &Case) -> bool {
        self.kind == case.kind
            && case
                .requirements
                .item_size
                .is_none_or(|size| self.item_size == Some(size))
            && (!case.requirements.requires_interior || self.containers > 0)
            && (!case.requirements.requires_audio || self.audio_triggers > 0)
            && (case.requirements.geometry_lod.is_none() || self.root_vertices.is_some())
    }
}

pub(super) fn prepare_with_audio<'f>(
    archive: &Archive,
    files: &'f dyn LoadoutFiles,
    entities: Arc<Entities<'f>>,
    record: &Record<'f>,
    canonical: &RecordTree<'f>,
    case: &Case,
    audio_triggers: usize,
) -> super::Result<Option<Prepared>> {
    let audio_only = case.requirements.requires_audio
        && case.requirements.geometry_lod.is_none()
        && case.requirements.item_size.is_none()
        && !case.requirements.requires_interior;
    let node = if audio_only {
        None
    } else {
        let observer = Arc::new(observer::Observer::default());
        let mut resolver =
            LoadoutResolver::with_observed_entities(entities, observer.clone()).with_files(files);
        if matches!(case.requirements.docked, DockedVehicles::Stop) {
            resolver = resolver.stopping_at_docked_vehicles();
        }
        let tree_result = resolver.resolve_tree(record, canonical);
        observer.finish()?;
        Some(tree_result?)
    };
    let geometry = geometry(archive, node.as_ref(), case.requirements.geometry_lod)?;
    let (actual_lods, root_vertices) = geometry.map_or_else(
        || (Vec::new(), None),
        |geometry| (geometry.lods, Some(geometry.vertices)),
    );
    let capabilities = Capabilities {
        kind: EntityKind::classify(
            record.record_path()?,
            node.as_ref().map_or("", EntityNode::attach_type),
        ),
        item_size: node
            .as_ref()
            .filter(|_| case.requirements.item_size.is_some())
            .and_then(|node| node.item_facts().size()),
        requested_lod: case.requirements.geometry_lod,
        actual_lods,
        root_vertices,
        containers: node
            .as_ref()
            .map_or(0, |node| node.object_containers().len()),
        audio_triggers,
    };
    Ok(capabilities
        .satisfies(case)
        .then_some(Prepared { node, capabilities }))
}

struct Geometry {
    lods: Vec<u32>,
    vertices: usize,
}

fn geometry(
    archive: &Archive,
    node: Option<&EntityNode>,
    lod: Option<u32>,
) -> super::Result<Option<Geometry>> {
    let Some(lod) = lod else { return Ok(None) };
    let Some(geometry) = node.and_then(EntityNode::geometry) else {
        return Ok(None);
    };
    let model = ArchiveModels::new(archive).model(geometry.path(), lod)?;
    let lods = model.model.lods.iter().map(|level| level.level).collect();
    let vertices = model.model.lods.iter().map(Lod::vertex_count).sum();
    Ok((vertices > 0).then_some(Geometry { lods, vertices }))
}
