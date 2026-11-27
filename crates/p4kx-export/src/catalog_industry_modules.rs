// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

pub(super) use super::{authored_fields, prepared_definition};

#[path = "blueprint_pool_definition.rs"]
pub(crate) mod blueprint_pool_definition;
#[path = "blueprint_pool_tables.rs"]
pub(crate) mod blueprint_pool_tables;
#[path = "blueprint_reward_source.rs"]
pub(crate) mod blueprint_reward_source;

#[path = "crafter_definition.rs"]
pub(crate) mod crafter_definition;
#[path = "crafter_host_projection.rs"]
pub(crate) mod crafter_host_projection;
#[path = "crafting_global.rs"]
pub(crate) mod crafting_global;
#[path = "dismantle_process_definition.rs"]
pub(crate) mod dismantle_process_definition;
#[path = "mineable_composition_definition.rs"]
pub(crate) mod mineable_composition_definition;
#[path = "mineable_element_definition.rs"]
pub(crate) mod mineable_element_definition;
#[path = "mining_process_catalog.rs"]
pub(crate) mod mining_process_catalog;

#[cfg(test)]
#[path = "blueprint_source_test_selectors.rs"]
mod blueprint_source_test_selectors;

#[path = "mineable_entity_definition.rs"]
pub(crate) mod mineable_entity_definition;
