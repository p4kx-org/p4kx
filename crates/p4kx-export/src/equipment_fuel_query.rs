// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{record_query, rows};

pub use super::ResourceCapacity;
pub use crate::catalog_v3::catalog_entity_configurations::{
    FuelAudioRtpc, FuelAudioTrigger, FuelIntake, FuelTank, HealthLink, ResourceCompositionEntry,
    ResourceContainer,
};

record_query::unlocalized!(
    rows::FuelTankRows,
    FuelTank,
    /// Original fuel tanks and their source evidence.
    FuelTankSnapshot,
    with_fuel_tanks,
    with_fuel_tanks_from
);

record_query::unlocalized!(
    rows::FuelIntakeRows,
    FuelIntake,
    /// Original fuel intakes and their source evidence.
    FuelIntakeSnapshot,
    with_fuel_intakes,
    with_fuel_intakes_from
);

record_query::unlocalized!(
    rows::ResourceContainerRows,
    ResourceContainer,
    /// Original resource containers and their source evidence.
    ResourceContainerSnapshot,
    with_resource_containers,
    with_resource_containers_from
);
