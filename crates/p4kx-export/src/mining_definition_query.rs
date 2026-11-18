// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "mineable_composition_query.rs"]
mod compositions;
#[path = "resource_query_dependencies.rs"]
mod dependencies;
#[path = "mineable_element_query.rs"]
mod elements;
use super::unlocalized_record_query::plan;
#[path = "mining_process_query.rs"]
pub mod process_api;
#[path = "mining_definition_read.rs"]
mod read;
#[path = "resource_type_query.rs"]
mod resources;
#[path = "mining_definition_rows.rs"]
mod rows;
#[path = "mining_definition_snapshot.rs"]
mod snapshot;

pub use compositions::{with_mineable_compositions, with_mineable_compositions_from};
pub use elements::{with_mineable_elements, with_mineable_elements_from};
pub use resources::{with_resource_types, with_resource_types_from};
pub use snapshot::{
    MineableCompositionSnapshot, MineableElementSnapshot, MiningDefinitionSnapshot,
    ResourceTypeSnapshot,
};

#[cfg(test)]
#[path = "mining_definition_query_tests.rs"]
mod tests;
