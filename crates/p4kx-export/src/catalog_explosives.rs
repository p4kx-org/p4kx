// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
#[path = "catalog_missile_configuration.rs"]
pub(crate) mod catalog_missile_configuration;
#[path = "primeable.rs"]
mod primeable;
#[path = "triggerable_device.rs"]
mod triggerable_device;

pub use super::authored_damage::{AuthoredDamage, DamageInfo, DamageVector};
pub use super::authored_effect_resource::{AudioResource, ParticleResource};
pub use super::authored_explosion::{
    ExplosionEffects, ExplosionGeometry, ExplosionParameters, Flashbang, Fragments, MaterialEffect,
    Shockwave,
};
use super::item_component_owner::ComponentOwner;
pub use super::ordnance_fields::{
    BombFields, OrdnanceAudio, OrdnanceCluster, OrdnanceEmission, OrdnanceFields,
};
pub use super::sensor_mine::{LaserTrigger, ProximityTrigger, SensorMine, SensorMineTrigger};
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use catalog_missile_configuration as missile;
pub use primeable::{AnimatedPrimeState, PrimeFlags, PrimeStates, Primeable};
pub use triggerable_device::{
    BehaviorEffect, DeviceBehavior, DeviceTrigger, LaserTrip, TriggerDetails, TriggerSource,
    TriggerableDevice,
};

const SENSOR_MINE: &str = "SSensorMineComponentParams";
const ORDNANCE: &str = "SExplosiveOrdnanceParams";
const BOMB: &str = "SCItemBombParams";
const SPACE_MINE: &str = "SCItemSpaceMineParams";

pub(super) const SOURCE_TYPES: &[&str] = &[
    SENSOR_MINE,
    ORDNANCE,
    BOMB,
    SPACE_MINE,
    missile::SOURCE_TYPE,
    triggerable_device::SOURCE_TYPE,
    primeable::SOURCE_TYPE,
];

#[derive(Clone, Copy)]
pub(super) enum ExplosiveComponent {
    SensorMine,
    Ordnance,
    Bomb,
    SpaceMine,
    Missile,
    TriggerableDevice,
    Primeable,
}

impl ExplosiveComponent {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            SENSOR_MINE => Some(Self::SensorMine),
            ORDNANCE => Some(Self::Ordnance),
            BOMB => Some(Self::Bomb),
            SPACE_MINE => Some(Self::SpaceMine),
            missile::SOURCE_TYPE => Some(Self::Missile),
            triggerable_device::SOURCE_TYPE => Some(Self::TriggerableDevice),
            primeable::SOURCE_TYPE => Some(Self::Primeable),
            _ => None,
        }
    }
}

/// One original explosive device component on its selected Item owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct Explosive<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    configuration: SourcedValue<ExplosiveConfiguration<'source>>,
}

impl<'source> Explosive<'source> {
    source_getters! {
        /// Borrows the authored explosive variant and its source evidence.
        configuration: SourcedValue<ExplosiveConfiguration<'source>>,
    }

    /// Borrows the original Item Record and physical Components slot.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the concrete authored component type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    pub(super) fn read(
        kind: ExplosiveComponent,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = source.source_type_value()?;
        let settings = ExplosiveConfiguration::read(kind, source, notes)?;
        let configuration = source_type.clone().map(|_| settings);
        Ok(Self {
            owner,
            source_type,
            configuration,
        })
    }

    pub(super) fn missile_ordnance(&self) -> Option<&OrdnanceFields<'source>> {
        match self.configuration.value()? {
            ExplosiveConfiguration::Missile { value } => Some(value),
            _ => None,
        }
    }
}

/// The device-specific shape selected by the actual component type.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub enum ExplosiveConfiguration<'source> {
    /// A sensor mine with an authored trigger.
    SensorMine {
        /// The original sensor mine settings.
        value: Box<SensorMine>,
    },
    /// A generic ordnance component.
    Ordnance {
        /// The original ordnance settings.
        value: Box<OrdnanceFields<'source>>,
    },
    /// A bomb with explicit aerodynamic settings.
    Bomb {
        /// The original bomb settings.
        value: Box<BombFields<'source>>,
    },
    /// A space mine.
    SpaceMine {
        /// The original inherited ordnance settings.
        value: Box<OrdnanceFields<'source>>,
    },
    /// A missile with inherited ordnance settings.
    Missile {
        /// The original inherited ordnance settings.
        value: Box<OrdnanceFields<'source>>,
    },
    /// A device with authored triggers and behavior.
    TriggerableDevice {
        /// The original triggerable-device settings.
        value: Box<TriggerableDevice<'source>>,
    },
    /// A device with authored priming behavior.
    Primeable {
        /// The original primeable settings.
        value: Box<Primeable<'source>>,
    },
}

impl<'source> ExplosiveConfiguration<'source> {
    fn read(
        kind: ExplosiveComponent,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match kind {
            ExplosiveComponent::SensorMine => Self::SensorMine {
                value: Box::new(SensorMine::read(source, notes)?),
            },
            ExplosiveComponent::Ordnance => Self::Ordnance {
                value: Box::new(OrdnanceFields::read(source, notes)?),
            },
            ExplosiveComponent::Bomb => Self::Bomb {
                value: Box::new(BombFields::read(source, notes)?),
            },
            ExplosiveComponent::SpaceMine => Self::SpaceMine {
                value: Box::new(OrdnanceFields::read(source, notes)?),
            },
            ExplosiveComponent::TriggerableDevice => Self::TriggerableDevice {
                value: Box::new(TriggerableDevice::read(source, notes)?),
            },
            ExplosiveComponent::Primeable => Self::Primeable {
                value: Box::new(Primeable::read(source, notes)?),
            },
            ExplosiveComponent::Missile => Self::Missile {
                value: Box::new(OrdnanceFields::read(source, notes)?),
            },
        })
    }
}

/// Retains physical Record and component order across every selected device.
pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<Explosive<'source>>,
) -> CatalogTable<Explosive<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, SOURCE_TYPES, rows)
}
