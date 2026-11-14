// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
#[path = "catalog_missile_configuration.rs"]
pub(crate) mod catalog_missile_configuration;

use super::item_component_owner::ComponentOwner;
use super::ordnance_fields::{BombFields, OrdnanceFields};
use super::sensor_mine::SensorMine;
use crate::CatalogError;
use crate::catalog_table::CatalogTable;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use catalog_missile_configuration as missile;

const SENSOR_MINE: &str = "SSensorMineComponentParams";
const ORDNANCE: &str = "SExplosiveOrdnanceParams";
const BOMB: &str = "SCItemBombParams";
const SPACE_MINE: &str = "SCItemSpaceMineParams";

#[derive(Clone, Copy)]
pub(super) enum ExplosiveComponent {
    SensorMine,
    Ordnance,
    Bomb,
    SpaceMine,
    Missile,
}

impl ExplosiveComponent {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            SENSOR_MINE => Some(Self::SensorMine),
            ORDNANCE => Some(Self::Ordnance),
            BOMB => Some(Self::Bomb),
            SPACE_MINE => Some(Self::SpaceMine),
            missile::SOURCE_TYPE => Some(Self::Missile),
            _ => None,
        }
    }
}

/// One original explosive device component on its selected Item owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Explosive<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    configuration: SourcedValue<ExplosiveConfiguration<'source>>,
}

impl<'source> Explosive<'source> {
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
enum ExplosiveConfiguration<'source> {
    SensorMine { value: Box<SensorMine> },
    Ordnance { value: Box<OrdnanceFields<'source>> },
    Bomb { value: Box<BombFields<'source>> },
    SpaceMine { value: Box<OrdnanceFields<'source>> },
    Missile { value: Box<OrdnanceFields<'source>> },
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
    CatalogTable::new(
        forge,
        &[
            SENSOR_MINE,
            ORDNANCE,
            BOMB,
            SPACE_MINE,
            missile::SOURCE_TYPE,
        ],
        rows,
    )
}
