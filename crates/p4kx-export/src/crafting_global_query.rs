// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Direct canonical crafting sources without unrelated body discovery.

#[path = "crafting_global_query_api.rs"]
mod api;
#[path = "crafting_global_query_read.rs"]
mod read;
#[path = "crafting_global_query_snapshot.rs"]
mod snapshot;

pub use crate::catalog_v3::crafting_global::{
    CraftingBlueprintSelection, CraftingGlobalConfiguration,
};
pub use api::{with_global_params, with_global_params_from};
pub use snapshot::CraftingGlobalSnapshot;

#[path = "blueprint_pool_query_api.rs"]
mod pool_api;
#[path = "blueprint_pool_query_read.rs"]
mod pool_read;
#[path = "blueprint_pool_query_snapshot.rs"]
mod pool_snapshot;

pub use crate::catalog_v3::industry_modules::blueprint_pool_definition::{
    BlueprintPoolDefinition, BlueprintPoolEntry, BlueprintPoolRef, BlueprintPoolWeights,
};
pub use crate::catalog_v3::industry_modules::blueprint_reward_source::BlueprintRewardSource;
pub use pool_api::{with_pools, with_pools_from};
pub use pool_snapshot::BlueprintPoolSnapshot;

#[cfg(test)]
#[path = "blueprint_pool_query_tests.rs"]
mod pool_tests;

#[cfg(test)]
pub(crate) use pool_tests::observe_pool_query;

#[cfg(test)]
#[path = "crafting_global_query_tests.rs"]
mod tests;

#[path = "blueprint_reward_query_api.rs"]
mod reward_api;
#[path = "blueprint_reward_query_coverage.rs"]
mod reward_coverage;
#[path = "blueprint_reward_query_read.rs"]
mod reward_read;
#[path = "blueprint_reward_query_snapshot.rs"]
mod reward_snapshot;

pub use reward_api::{with_reward_sources, with_reward_sources_from};
pub use reward_snapshot::{BlueprintRewardSelection, BlueprintRewardSnapshot};

#[cfg(test)]
#[path = "blueprint_reward_query_tests.rs"]
mod reward_tests;
