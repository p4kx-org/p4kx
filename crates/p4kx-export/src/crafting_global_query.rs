// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! Direct canonical crafting globals without Blueprint or resource discovery.

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

#[cfg(test)]
#[path = "crafting_global_query_tests.rs"]
mod tests;
