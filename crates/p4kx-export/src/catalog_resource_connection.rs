// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_resource_heat_connection::ResourceHeatConnection;
use super::catalog_resource_power_connection::ResourcePowerConnection;
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

const POWER: &str = "EntityComponentPowerConnection";
const HEAT: &str = "EntityComponentHeatConnection";
pub(super) const SOURCE_TYPES: &[&str] = &[POWER, HEAT];

#[derive(Clone, Copy)]
pub(super) enum ConnectionKind {
    Power,
    Heat,
}

impl ConnectionKind {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            POWER => Some(Self::Power),
            HEAT => Some(Self::Heat),
            _ => None,
        }
    }
}

/// An original power or heat connection with its physical component owner.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct ResourceConnection<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: ConnectionFields<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum ConnectionFields<'source> {
    Power(ResourcePowerConnection<'source>),
    Heat(ResourceHeatConnection<'source>),
}

impl<'source> ResourceConnection<'source> {
    pub(super) fn read(
        kind: ConnectionKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let fields = match kind {
            ConnectionKind::Power => {
                ConnectionFields::Power(ResourcePowerConnection::read(source, notes)?)
            }
            ConnectionKind::Heat => {
                ConnectionFields::Heat(ResourceHeatConnection::read(source, notes)?)
            }
        };
        Ok(Self {
            owner,
            source_type: source.source_type_value()?,
            fields,
        })
    }

    pub(super) const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }
}
