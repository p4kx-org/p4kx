// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_model::{TintPalette, Transform};

use super::{Library, RenderedGeometry};
use crate::build_place::PlacedPart;
use crate::flatten::Draft;
use crate::{AttachedModel, Part};

pub(super) fn build_part(
    draft: &Draft<'_>,
    placement: PlacedPart,
    world: Transform,
    geometry: RenderedGeometry,
    (materials, from_model): (Library, bool),
    palette: Option<Arc<TintPalette>>,
    attachments: Vec<AttachedModel>,
) -> Part {
    Part {
        name: draft.name.clone(),
        path: draft.path.clone(),
        guid: draft.node.guid(),
        entity_record_index: if draft.gear.is_none() {
            draft.node.record_index()
        } else {
            None
        },
        class_name: draft.class_name().to_owned(),
        classification: draft
            .gear
            .is_none()
            .then(|| draft.node.classification().clone()),
        landing_gear: placement.gear,
        parent: draft.parent,
        local: placement.local,
        world,
        placement: placement.kind,
        placement_source: placement.port,
        model_placement: placement.nodes,
        geometry: draft.geometry_path().map(str::to_owned),
        model: geometry.model,
        model_archive_sources: geometry.sources,
        model_definition: geometry.definition.with_model_material(from_model),
        geometry_selectors: geometry.selectors,
        materials,
        palette,
        attachments,
        hidden: draft.hidden,
    }
}
