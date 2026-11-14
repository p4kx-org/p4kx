// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::cargo_source;
use super::catalog_capacity::ResourceCapacity;
use super::catalog_unit_source::UnitSource;
use super::inventory_open_container::InventoryOpenContainer;
use crate::CatalogError;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const BASE: &str = "InventoryContainerTypeBase";

/// Concrete authored storage policy.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) enum InventoryContainerType<'source> {
    Open {
        #[serde(flatten)]
        source: Box<InventoryOpenContainer<'source>>,
    },
    Closed {
        #[serde(flatten)]
        source: UnitSource<'source>,
        capacity: SourcedValue<ResourceCapacity<'source>>,
    },
    Base {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
    UnknownType {
        #[serde(flatten)]
        source: UnitSource<'source>,
    },
}

impl<'source> InventoryContainerType<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(match source.source_type()? {
            cargo_source::OPEN => Self::Open {
                source: Box::new(InventoryOpenContainer::read(source, notes)?),
            },
            cargo_source::CLOSED => {
                let capacity = source.strong_pointer_family_to(
                    "capacity",
                    cargo_source::BASE,
                    notes,
                    ResourceCapacity::read,
                )?;
                let source = UnitSource::read(source, [("capacity", capacity.state())])?;
                Self::Closed { source, capacity }
            }
            BASE => Self::Base {
                source: UnitSource::read(source, [])?,
            },
            _ => Self::UnknownType {
                source: UnitSource::read(source, [])?,
            },
        })
    }
}
