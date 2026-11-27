// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::cargo_source;
use super::super::catalog_capacity::ResourceCapacity;
use super::super::catalog_unit_source::UnitSource;
use super::super::interaction_source::RecordLink;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceRecordReference;

pub(super) const SOURCE_TYPE: &str = "MedBedResourceConsumptionParams";
const RESOURCE_TYPE: &str = "ResourceType";
const SURGERY_T1: &str = "resourceAmountForSurgeryT1";
const SURGERY_T2: &str = "resourceAmountForSurgeryT2";
const SURGERY_T3: &str = "resourceAmountForSurgeryT3";
const CLONING: &str = "resourceAmountForCloning";

pub(super) type Capacity<'source> = SourcedValue<ResourceCapacity<'source>>;

/// One original medical facility resource consumption declaration.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct MedBedResourceConsumption<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    bed_resource: RecordLink<'source>,
    #[serde(rename = "resourceAmountForSurgeryT1")]
    surgery_t1: Capacity<'source>,
    #[serde(rename = "resourceAmountForSurgeryT2")]
    surgery_t2: Capacity<'source>,
    #[serde(rename = "resourceAmountForSurgeryT3")]
    surgery_t3: Capacity<'source>,
    #[serde(rename = "resourceAmountForCloning")]
    cloning: Capacity<'source>,
}

impl<'source> MedBedResourceConsumption<'source> {
    /// Returns the original class and additional authored fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    /// Returns the authored resource reference.
    #[must_use]
    pub const fn bed_resource(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.bed_resource
    }

    /// Returns the authored resource amount for tier-one surgery.
    #[must_use]
    pub const fn surgery_t1(&self) -> &SourcedValue<ResourceCapacity<'source>> {
        &self.surgery_t1
    }

    /// Returns the authored resource amount for tier-two surgery.
    #[must_use]
    pub const fn surgery_t2(&self) -> &SourcedValue<ResourceCapacity<'source>> {
        &self.surgery_t2
    }

    /// Returns the authored resource amount for tier-three surgery.
    #[must_use]
    pub const fn surgery_t3(&self) -> &SourcedValue<ResourceCapacity<'source>> {
        &self.surgery_t3
    }

    /// Returns the authored resource amount for cloning.
    #[must_use]
    pub const fn cloning(&self) -> &SourcedValue<ResourceCapacity<'source>> {
        &self.cloning
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let bed_resource = source.record_link_to("bedResource", RESOURCE_TYPE, notes)?;
        let surgery_t1 = capacity(source, SURGERY_T1, notes)?;
        let surgery_t2 = capacity(source, SURGERY_T2, notes)?;
        let surgery_t3 = capacity(source, SURGERY_T3, notes)?;
        let cloning = capacity(source, CLONING, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("bedResource", bed_resource.state()),
                (SURGERY_T1, surgery_t1.state()),
                (SURGERY_T2, surgery_t2.state()),
                (SURGERY_T3, surgery_t3.state()),
                (CLONING, cloning.state()),
            ],
        )?;
        Ok(Self {
            source,
            bed_resource,
            surgery_t1,
            surgery_t2,
            surgery_t3,
            cloning,
        })
    }
}

pub(super) fn capacity<'source>(
    source: Observed<'_, '_, '_, 'source>,
    field: &str,
    notes: &mut Vec<FieldError>,
) -> Result<Capacity<'source>, CatalogError> {
    source.strong_pointer_family_to(field, cargo_source::BASE, notes, ResourceCapacity::read)
}
