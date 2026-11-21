// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

#[path = "flight_component_sources.rs"]
pub(crate) mod flight;
#[path = "physical_component_sources.rs"]
mod physical;

pub(crate) use physical::PhysicalComponent;
pub(super) use physical::physical_component;
