// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "build_catalog.rs"]
pub(crate) mod api;
pub use api::{build_catalog, build_catalog_from, write_catalog, write_catalog_from};
#[path = "catalog_queries.rs"]
pub mod catalog_queries;
#[path = "build_source.rs"]
mod metadata;
#[path = "build_mining_locations.rs"]
pub(crate) mod mining_locations;
#[path = "original_catalog_inputs.rs"]
pub(crate) mod original_input;
#[path = "build_parts.rs"]
mod parts;
#[path = "catalog_refusal.rs"]
pub(crate) mod refusal;
pub(crate) use metadata::source;
#[path = "build_assembly.rs"]
mod assembly;
#[path = "build_scan.rs"]
mod scanning;
#[path = "build_standalone_ships.rs"]
pub(crate) mod standalone_ships;

#[cfg(test)]
#[path = "installed_ship_source_projection.rs"]
pub(crate) mod installed_ship_projection;

#[cfg(test)]
#[path = "installed_loot_delta_witness.rs"]
mod installed_loot_delta_witness;
