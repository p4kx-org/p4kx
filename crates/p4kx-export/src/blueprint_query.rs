// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "blueprint_query_api.rs"]
mod api;
#[path = "blueprint_query_categories.rs"]
mod categories;
#[path = "blueprint_query_dependencies.rs"]
mod dependencies;
#[path = "blueprint_query_plan.rs"]
mod plan;
#[path = "blueprint_query_project.rs"]
mod project;
#[path = "blueprint_query_read.rs"]
mod read;
#[path = "blueprint_query_snapshot.rs"]
mod snapshot;

pub use api::{with_blueprints, with_blueprints_from};
pub use snapshot::{BlueprintDependencyCoverage, BlueprintSnapshot, BlueprintUnsupportedRanges};

pub use crate::blueprint_context_contract::{
    BlueprintCostContext, BlueprintModifier, BlueprintModifierRange, BlueprintRangeKind,
};
pub use crate::blueprint_definition::{BlueprintDefinition, PhysicalBlueprintRef};
pub use crate::blueprint_presentation::{
    BlueprintDismantle, BlueprintIngredient, BlueprintPresentation,
};
pub use crate::blueprint_research_contract::{
    BlueprintResearch, BlueprintResearchEnvelope, BlueprintResearchPointer,
};
pub use crate::blueprint_tier_contract::{
    BlueprintCost, BlueprintCostMeaning, BlueprintCostPosition, BlueprintTier,
};

#[cfg(test)]
#[path = "blueprint_query_tests.rs"]
mod tests;
