// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::EquipmentSnapshot;
use super::component_query::equipment_component_query;
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::catalog_independent_configurations::entity_animation_controller::EntityAnimationControllers;

pub use super::ComponentOwner;
pub use crate::catalog_v3::catalog_independent_configurations::entity_animation_controller::EntityAnimationController;

equipment_component_query!(
    EntityAnimationControllers,
    EntityAnimationController,
    EntityAnimationControllerSnapshot,
    with_entity_animation_controllers,
    with_entity_animation_controllers_from
);
