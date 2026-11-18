// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::interaction_source::RecordLink;
use super::super::inventory_dimensions::InventoryVector3;
use super::CargoGridCapacity;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceArenaOutput, SourceRecordReference};

pub(super) const COMPONENT: &str = "SCItemCargoGridParams";
const DAMAGE: &str = "CommodityDamageConfiguration";

/// Original configuration of one cargo grid component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct CargoGrid<'source> {
    #[serde(serialize_with = "serialize_capacity")]
    #[schemars(with = "CargoGridCapacity")]
    pub(super) capacity: Option<CargoGridCapacity>,
    dimensions: SourcedValue<InventoryVector3<'source>>,
    crate_gen_percentage_on_destroy: SourcedValue<ResourceFloat32>,
    crate_max_on_destroy: SourcedValue<i32>,
    invisible: SourcedValue<bool>,
    mining_only: SourcedValue<bool>,
    transform_dependant: SourcedValue<bool>,
    min_volatile_power_to_explode: SourcedValue<ResourceFloat32>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    damage_configuration: RecordLink<'source>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> CargoGrid<'source> {
    /// Borrows the resolved capacity and its physical inventory bindings.
    #[must_use]
    pub const fn capacity(&self) -> Option<&CargoGridCapacity> {
        self.capacity.as_ref()
    }

    /// Borrows the authored grid dimensions.
    #[must_use]
    pub const fn dimensions(&self) -> &SourcedValue<InventoryVector3<'source>> {
        &self.dimensions
    }

    /// Borrows the authored destruction crate percentage.
    #[must_use]
    pub const fn crate_gen_percentage_on_destroy(&self) -> &SourcedValue<ResourceFloat32> {
        &self.crate_gen_percentage_on_destroy
    }

    /// Borrows the authored destruction crate limit.
    #[must_use]
    pub const fn crate_max_on_destroy(&self) -> &SourcedValue<i32> {
        &self.crate_max_on_destroy
    }

    /// Borrows the original visibility flag.
    #[must_use]
    pub const fn invisible(&self) -> &SourcedValue<bool> {
        &self.invisible
    }

    /// Borrows the original mining-only flag.
    #[must_use]
    pub const fn mining_only(&self) -> &SourcedValue<bool> {
        &self.mining_only
    }

    /// Borrows the original transform-dependent flag.
    #[must_use]
    pub const fn transform_dependant(&self) -> &SourcedValue<bool> {
        &self.transform_dependant
    }

    /// Borrows the authored volatile-explosion threshold.
    #[must_use]
    pub const fn min_volatile_power_to_explode(&self) -> &SourcedValue<ResourceFloat32> {
        &self.min_volatile_power_to_explode
    }

    /// Borrows the original damage configuration Reference.
    #[must_use]
    pub const fn damage_configuration(&self) -> &SourcedValue<Box<SourceRecordReference<'source>>> {
        &self.damage_configuration
    }

    /// Borrows the other authored component members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let dimensions = InventoryVector3::class_field(source, "dimensions", notes)?;
        let crate_gen_percentage_on_destroy =
            source.single("crateGenPercentageOnDestroy", notes)?;
        let crate_max_on_destroy = source.int32("crateMaxOnDestroy", notes)?;
        let invisible = source.boolean("invisible", notes)?;
        let mining_only = source.boolean("miningOnly", notes)?;
        let transform_dependant = source.boolean("transformDependant", notes)?;
        let min_volatile_power_to_explode = source.single("minVolatilePowerToExplode", notes)?;
        let damage_configuration = source.record_link_to("damageConfiguration", DAMAGE, notes)?;
        let additional_fields = source.authored_members_except([
            ("dimensions", dimensions.state()),
            (
                "crateGenPercentageOnDestroy",
                crate_gen_percentage_on_destroy.state(),
            ),
            ("crateMaxOnDestroy", crate_max_on_destroy.state()),
            ("invisible", invisible.state()),
            ("miningOnly", mining_only.state()),
            ("transformDependant", transform_dependant.state()),
            (
                "minVolatilePowerToExplode",
                min_volatile_power_to_explode.state(),
            ),
            ("damageConfiguration", damage_configuration.state()),
        ])?;
        Ok(Self {
            capacity: None,
            dimensions,
            crate_gen_percentage_on_destroy,
            crate_max_on_destroy,
            invisible,
            mining_only,
            transform_dependant,
            min_volatile_power_to_explode,
            damage_configuration,
            additional_fields,
        })
    }
}

#[expect(
    clippy::ref_option,
    reason = "Serde passes the unfinished field by reference to the custom serializer"
)]
fn serialize_capacity<S: serde::Serializer>(
    capacity: &Option<CargoGridCapacity>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    capacity
        .as_ref()
        .ok_or_else(|| <S::Error as serde::ser::Error>::custom("unreconciled cargo grid capacity"))?
        .serialize(serializer)
}
