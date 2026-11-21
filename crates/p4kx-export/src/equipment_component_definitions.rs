// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{Archive, CancelToken, DataForgeEntry};

use super::{EquipmentDependencyCoverage, EquipmentSnapshot};
use crate::CatalogError;
use crate::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, CatalogRecordSelection, ParsedCatalogQueries,
};
use crate::catalog_v3::component_definition_sources::{
    ArmorMacros, ConsumableSubtypes, ConsumableTypes, GlobalJumpConfigurations,
    JumpDriveFlightTuning, JumpTunnelForces, MiningLaserGlobals, TurretModifiers,
};

pub use crate::catalog_v3::catalog_independent_configurations::{
    AuthoredList as ConsumableList, ConsumableEffect, ConsumableSubtype, ConsumableType,
    DamageResistanceMacro, DurabilityResistanceEntry,
    EffectParameters as ConsumableEffectParameters, GasMass as ConsumableGasMass,
    GlobalJumpConfiguration, MacroBody as DamageResistanceMacroBody, MiningLaserGlobalDefinition,
    OrderedCurve, Override as ConsumableBuffOverride, ResistanceSet, Rtpc, TurretHealthModifier,
};

pub use crate::catalog_v3::component_definition_sources::JumpDriveDefinition;

#[path = "equipment_component_definition_read.rs"]
mod read;
use read::Query;

/// Selected definition Records, canonical source cells and discovery coverage.
pub type ComponentDefinitionSnapshot<'source, Row> =
    EquipmentSnapshot<'source, Row, EquipmentDependencyCoverage>;

impl<Row> ComponentDefinitionSnapshot<'_, Row> {
    /// Borrows the source coverage of required definition-family discovery.
    #[must_use]
    pub const fn definition_discovery(&self) -> &EquipmentDependencyCoverage {
        &self.dependency_coverage
    }
}

macro_rules! definition_query {
    ($family:ty, $row:ident, $snapshot:ident, $with:ident, $from:ident, $policy:expr; $($language:ident)?) => {
        /// Selected canonical rows and original source evidence for this definition family.
        pub type $snapshot<'source> = ComponentDefinitionSnapshot<'source, $row<'source>>;

        /// Reads selected physical definitions from the Archive's DataForge.
        ///
        /// # Errors
        /// Returns source evidence, Archive change or cancellation errors.
        pub fn $with<T>(
            archive: &Archive,
            selection: CatalogRecordSelection,
            $($language: CatalogLanguageSelection<'_>,)?
            cancel: Option<&CancelToken>,
            inspect: impl FnOnce($snapshot<'_>) -> T,
        ) -> Result<T, CatalogError> {
            CatalogQuerySession::open(archive, $policy, cancel)?.$with(selection, cancel, inspect)
        }

        /// Reads selected definitions using the Archive's already decoded DataForge.
        ///
        /// # Errors
        /// Returns query errors or a source-origin mismatch for a foreign Entry.
        pub fn $from<T>(
            archive: &Archive,
            entry: &DataForgeEntry,
            selection: CatalogRecordSelection,
            $($language: CatalogLanguageSelection<'_>,)?
            cancel: Option<&CancelToken>,
            inspect: impl FnOnce($snapshot<'_>) -> T,
        ) -> Result<T, CatalogError> {
            CatalogQuerySession::from_entry(archive, entry, $policy, cancel)?
                .$with(selection, cancel, inspect)
        }

        impl CatalogQuerySession<'_> {
            /// Reads selected definitions through this Archive session.
            ///
            /// # Errors
            /// Returns query errors, Archive change or cancellation.
            pub fn $with<T>(
                &self,
                selection: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
                inspect: impl FnOnce($snapshot<'_>) -> T,
            ) -> Result<T, CatalogError> {
                self.parsed(cancel)?.$with(selection, cancel, inspect)
            }
        }

        impl ParsedCatalogQueries<'_> {
            /// Reads selected definitions through this parsed DataForge view.
            ///
            /// # Errors
            /// Returns query errors, Archive change or cancellation.
            pub fn $with<T>(
                &self,
                selection: CatalogRecordSelection,
                cancel: Option<&CancelToken>,
                inspect: impl FnOnce($snapshot<'_>) -> T,
            ) -> Result<T, CatalogError> {
                self.project(cancel, |archive, entry, forge| {
                    Query { archive, entry, forge, selection, languages: self.languages(), cancel }
                        .read::<$family, T>(inspect)
                })
            }
        }
    };
}

definition_query!(
    JumpDriveFlightTuning, JumpDriveDefinition, JumpDriveFlightTuningSnapshot,
    with_jump_drive_flight_tuning, with_jump_drive_flight_tuning_from,
    CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    JumpTunnelForces, JumpDriveDefinition, JumpTunnelForceSnapshot,
    with_jump_tunnel_forces, with_jump_tunnel_forces_from,
    CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    GlobalJumpConfigurations, GlobalJumpConfiguration, GlobalJumpConfigurationSnapshot,
    with_global_jump_point_configurations, with_global_jump_point_configurations_from,
    CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    ArmorMacros, DamageResistanceMacro, DamageResistanceMacroSnapshot, with_damage_resistance_macros,
    with_damage_resistance_macros_from, CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    MiningLaserGlobals, MiningLaserGlobalDefinition, MiningLaserGlobalSnapshot, with_mining_laser_globals,
    with_mining_laser_globals_from, CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    TurretModifiers, TurretHealthModifier, TurretHealthModifierSnapshot, with_turret_health_modifiers,
    with_turret_health_modifiers_from, CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    ConsumableTypes, ConsumableType, ConsumableTypeSnapshot, with_consumable_types,
    with_consumable_types_from, CatalogLanguageSelection::Selected(&[]);
);
definition_query!(
    ConsumableSubtypes, ConsumableSubtype, ConsumableSubtypeSnapshot, with_consumable_subtypes,
    with_consumable_subtypes_from, languages; languages
);

#[path = "equipment_game_mode_query.rs"]
pub(in crate::build::catalog_queries) mod game_modes;
#[path = "weather_query.rs"]
pub(in crate::build::catalog_queries) mod weather;
