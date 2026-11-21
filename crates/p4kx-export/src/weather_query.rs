// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{ComponentDefinitionSnapshot, Query};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::component_definition_sources::ElectricalBehaviors;

pub use crate::catalog_v3::catalog_independent_configurations::electrical_behavior::{
    ElectricalBehavior, InternalPlanetLightning, LightningBehavior, LightningEffect,
    LightningRange, LightningTargetMode, LightningTargetPayload,
};

definition_query!(
    ElectricalBehaviors, ElectricalBehavior, ElectricalBehaviorSnapshot, with_electrical_behaviors,
    with_electrical_behaviors_from, CatalogLanguageSelection::Selected(&[]);
);
