// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, RecordTree};

use super::record_query;
use super::rows::{Family, Projector, ReadOptions};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::scoped_source_read::ScopedSourceRead;
use crate::catalog_v3::catalog_item_components::ItemEquipmentFamily;
use crate::catalog_v3::explosion_definition;
use crate::record_observation::RecordObservation;

pub use crate::catalog_v3::catalog_emp::{Emp, EmpParameters, EmpParticle, EmpStages, EmpTriggers};
pub use crate::catalog_v3::catalog_explosives::catalog_missile_configuration::{
    MissileAxes, MissileConfiguration, MissileGuidance, MissileGuidancePhase, MissileShake,
    MissileTargeting,
};
pub use crate::catalog_v3::catalog_explosives::{
    AnimatedPrimeState, AudioResource, AuthoredDamage, BehaviorEffect, BombFields, DamageInfo,
    DamageVector, DeviceBehavior, DeviceTrigger, ExplosionEffects, ExplosionGeometry,
    ExplosionParameters, Explosive, ExplosiveConfiguration, Flashbang, Fragments, LaserTrigger,
    LaserTrip, MaterialEffect, OrdnanceAudio, OrdnanceCluster, OrdnanceEmission, OrdnanceFields,
    ParticleResource, PrimeFlags, PrimeStates, Primeable, ProximityTrigger, SensorMine,
    SensorMineTrigger, Shockwave, TriggerDetails, TriggerSource, TriggerableDevice,
};
pub use crate::catalog_v3::explosion_definition::ExplosionDefinition;

record_query::item_component!(EmpRows, 'source, Emp<'source>, ItemEquipmentFamily::Emp, read_emp);
record_query::item_component!(
    ExplosiveRows, 'source, Explosive<'source>, ItemEquipmentFamily::Explosives, read_explosives
);
record_query::item_component!(
    MissileRows, 'source, MissileConfiguration<'source>, ItemEquipmentFamily::MissileConfigurations,
    read_missile_configurations
);

struct ExplosionDefinitionRows;

impl Projector for ExplosionDefinitionRows {
    type Row<'source> = ExplosionDefinition<'source>;
    const FAMILY: Family = Family::ExplosionDefinitions;

    fn read<'source>(
        _: &'source DataForge<'source>,
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        _: &ReadOptions<'_>,
        cancel: Option<&CancelToken>,
    ) -> Result<ScopedSourceRead<Vec<Self::Row<'source>>>, CatalogError> {
        let read = explosion_definition::read(tree, projection, admission, cancel)?;
        Ok(ScopedSourceRead::new(
            vec![read.value.into_definition()],
            read.notes,
            true,
        ))
    }
}

record_query::unlocalized!(
    EmpRows,
    Emp,
    /// Selected EMP components and their original source evidence.
    EmpSnapshot,
    with_emp,
    with_emp_from
);
record_query::unlocalized!(
    ExplosiveRows,
    Explosive,
    /// Selected explosive components and their original source evidence.
    ExplosiveSnapshot,
    with_explosives,
    with_explosives_from
);
record_query::unlocalized!(
    MissileRows,
    MissileConfiguration,
    /// Selected missile configurations and their original source evidence.
    MissileConfigurationSnapshot,
    with_missile_configurations,
    with_missile_configurations_from
);
record_query::unlocalized!(
    ExplosionDefinitionRows,
    ExplosionDefinition,
    /// Selected standalone explosion definitions and their original source evidence.
    ExplosionDefinitionSnapshot,
    with_explosion_definitions,
    with_explosion_definitions_from
);
