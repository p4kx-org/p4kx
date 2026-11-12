// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_seat::{self, Seat};
use super::item_component_owner::ComponentOwner;
use super::medical::med_bed::{self, MedBed};
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPES: &[&str] = &[catalog_seat::COMPONENT, med_bed::COMPONENT];

#[derive(Clone, Copy)]
pub(super) enum FacilityKind {
    Seat,
    MedBed,
}

impl FacilityKind {
    pub(super) fn of(source_type: &str) -> Option<Self> {
        match source_type {
            catalog_seat::COMPONENT => Some(Self::Seat),
            med_bed::COMPONENT => Some(Self::MedBed),
            _ => None,
        }
    }
}

/// The configuration of one original facility component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct Facility<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    fields: FacilityFields<'source>,
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum FacilityFields<'source> {
    Seat(Box<Seat<'source>>),
    MedBed(Box<MedBed<'source>>),
}

impl<'source> Facility<'source> {
    pub(super) fn read(
        kind: FacilityKind,
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let fields = match kind {
            FacilityKind::Seat => FacilityFields::Seat(Box::new(Seat::read(source, notes)?)),
            FacilityKind::MedBed => {
                FacilityFields::MedBed(Box::new(MedBed::read(source, projection, notes)?))
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
