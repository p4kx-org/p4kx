// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;

use p4kx_assembly::PortDefinitionPosition;
use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::ItemAlignmentRows;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::schema_fields::every_field_required;
use crate::{CatalogError, ExportAccountingError};

#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "state",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
#[schemars(transform = every_field_required)]
pub(crate) enum PortDefinitionLink {
    Projected {
        #[serde(flatten)]
        owner: ComponentOwner,
        port_ordinal: SourcedValue<u32>,
    },
    NotProjected,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "state", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) enum AttachmentDefinitionLink {
    Projected {
        #[serde(flatten)]
        owner: ComponentOwner,
    },
    NotProjected,
    NotDeclared,
}

#[derive(Default)]
pub(crate) struct AlignmentIndex<'rows> {
    ports: HashMap<(u32, u32, u32), PortOwner<'rows>>,
    items: HashMap<(u32, u32), &'rows ComponentOwner>,
}

struct PortOwner<'rows> {
    component: &'rows ComponentOwner,
    port_ordinal: &'rows SourcedValue<u32>,
}

impl<'rows> AlignmentIndex<'rows> {
    pub(crate) fn new(rows: &'rows ItemAlignmentRows<'_>) -> Result<Self, CatalogError> {
        let mut this = Self::default();
        for row in &rows.ports {
            let (record, component) = physical_key(&row.owner)?;
            for port in row
                .ports
                .value()
                .into_iter()
                .flatten()
                .filter_map(SourcedValue::value)
            {
                let ordinal = port
                    .port_ordinal
                    .value()
                    .copied()
                    .ok_or(ExportAccountingError::ConflictingSourceDatum)?;
                let source = PortOwner {
                    component: &row.owner,
                    port_ordinal: &port.port_ordinal,
                };
                if this
                    .ports
                    .insert((record, component, ordinal), source)
                    .is_some()
                {
                    return Err(ExportAccountingError::ConflictingSourceDatum.into());
                }
            }
        }
        for row in &rows.attachments {
            let key = physical_key(&row.owner)?;
            if this.items.insert(key, &row.owner).is_some() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
        }
        Ok(this)
    }

    pub(crate) fn port(
        &self,
        position: PortDefinitionPosition,
    ) -> Result<PortDefinitionLink, CatalogError> {
        let key = (
            position.record_index(),
            ordinal(position.component_ordinal())?,
            ordinal(position.port_ordinal())?,
        );
        Ok(self
            .ports
            .get(&key)
            .map_or(PortDefinitionLink::NotProjected, |found| {
                PortDefinitionLink::Projected {
                    owner: found.component.clone(),
                    port_ordinal: found.port_ordinal.clone(),
                }
            }))
    }

    pub(crate) fn attachment(
        &self,
        record: u32,
        component: Option<usize>,
    ) -> Result<AttachmentDefinitionLink, CatalogError> {
        let Some(component) = component else {
            return Ok(AttachmentDefinitionLink::NotDeclared);
        };
        Ok(self.items.get(&(record, ordinal(component)?)).map_or(
            AttachmentDefinitionLink::NotProjected,
            |owner| AttachmentDefinitionLink::Projected {
                owner: (*owner).clone(),
            },
        ))
    }
}

fn ordinal(value: usize) -> Result<u32, ExportAccountingError> {
    u32::try_from(value).map_err(|_| ExportAccountingError::CounterOverflow)
}

fn physical_key(owner: &ComponentOwner) -> Result<(u32, u32), ExportAccountingError> {
    let (record, component) = owner.key();
    record
        .zip(component)
        .ok_or(ExportAccountingError::ConflictingSourceDatum)
}
