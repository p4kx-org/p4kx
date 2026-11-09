// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "gameplay_property_contract.rs"]
mod contract;
#[path = "gameplay_property_overrides.rs"]
mod overrides;
#[path = "gameplay_property_read.rs"]
mod read;
#[path = "gameplay_property_rows.rs"]
mod rows;
#[path = "gameplay_transformation.rs"]
mod transformation;
#[path = "gameplay_transformation_contract.rs"]
mod transformation_contract;

pub(crate) use contract::GameplayPropertyDefinition;
pub(crate) use read::{read, table};
pub(crate) use rows::{GameplayPropertyInput, GameplayPropertyRows};
