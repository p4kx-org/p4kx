// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "owned_array.rs"]
mod array;
#[path = "build_bytes.rs"]
mod bytes;
#[path = "owned_decode.rs"]
mod decode;
#[path = "owned_driver.rs"]
mod driver;
#[path = "owned_frame.rs"]
mod frame;
#[path = "materialization_budget.rs"]
mod materialization_budget;
#[path = "owned_positions.rs"]
mod positions;
#[path = "owned_task.rs"]
mod task;
#[path = "tree_builder.rs"]
mod tree_builder;

pub(crate) use bytes::{element_bytes, pointer, reference_guid, words};
pub use tree_builder::TreeBuilder;

pub(crate) use positions::Positions;

#[cfg(test)]
#[path = "owned_cost_tests.rs"]
mod tests;
