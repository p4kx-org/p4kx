// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::Observed;
use crate::CatalogError;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const COMPONENT: &str = "SCItemRadarComponentParams";

/// One original radar component installed on a physical entity record.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ItemRadar<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    aim_assist: SourcedValue<AimAssist<'source>>,
    ping_properties: SourcedValue<PingProperties<'source>>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> ItemRadar<'source> {
    /// Borrows the original physical component owner.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original inline aim assist settings.
    #[must_use]
    pub const fn aim_assist(&self) -> &SourcedValue<AimAssist<'source>> {
        &self.aim_assist
    }

    /// Borrows the original inline ping settings.
    #[must_use]
    pub const fn ping_properties(&self) -> &SourcedValue<PingProperties<'source>> {
        &self.ping_properties
    }

    /// Borrows original type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.original
    }

    pub(super) fn read(
        owner: ComponentOwner,
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let aim_assist =
            source.declared_class_to("aimAssist", "SRadarAimAssistData", notes, AimAssist::read)?;
        let ping_properties = source.declared_class_to(
            "pingProperties",
            "SCItemRadarPingProperties",
            notes,
            PingProperties::read,
        )?;
        let original = UnitSource::read(
            source,
            [
                ("aimAssist", aim_assist.state()),
                ("pingProperties", ping_properties.state()),
            ],
        )?;
        Ok(Self {
            owner,
            aim_assist,
            ping_properties,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original radar aim assist values and remaining inline fields.
pub struct AimAssist<'source> {
    distance_min_assignment: SourcedValue<ResourceFloat32>,
    distance_max_assignment: SourcedValue<ResourceFloat32>,
    outside_range_buffer_distance: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> AimAssist<'source> {
    /// Borrows the original minimum assignment distance.
    #[must_use]
    pub const fn distance_min_assignment(&self) -> &SourcedValue<ResourceFloat32> {
        &self.distance_min_assignment
    }

    /// Borrows the original maximum assignment distance.
    #[must_use]
    pub const fn distance_max_assignment(&self) -> &SourcedValue<ResourceFloat32> {
        &self.distance_max_assignment
    }

    /// Borrows the original buffer beyond assignment range.
    #[must_use]
    pub const fn outside_range_buffer_distance(&self) -> &SourcedValue<ResourceFloat32> {
        &self.outside_range_buffer_distance
    }

    /// Borrows original type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.original
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let distance_min_assignment = source.single("distanceMinAssignment", notes)?;
        let distance_max_assignment = source.single("distanceMaxAssignment", notes)?;
        let outside_range_buffer_distance = source.single("outsideRangeBufferDistance", notes)?;
        let original = UnitSource::read(
            source,
            [
                ("distanceMinAssignment", distance_min_assignment.state()),
                ("distanceMaxAssignment", distance_max_assignment.state()),
                (
                    "outsideRangeBufferDistance",
                    outside_range_buffer_distance.state(),
                ),
            ],
        )?;
        Ok(Self {
            distance_min_assignment,
            distance_max_assignment,
            outside_range_buffer_distance,
            original,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
/// Original radar ping settings and remaining inline fields.
pub struct PingProperties<'source> {
    cooldown_time: SourcedValue<ResourceFloat32>,
    #[serde(flatten)]
    original: UnitSource<'source>,
}

impl<'source> PingProperties<'source> {
    /// Borrows the original cooldown time.
    #[must_use]
    pub const fn cooldown_time(&self) -> &SourcedValue<ResourceFloat32> {
        &self.cooldown_time
    }

    /// Borrows original type and remaining fields.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.original
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let cooldown_time = source.single("cooldownTime", notes)?;
        let original = UnitSource::read(source, [("cooldownTime", cooldown_time.state())])?;
        Ok(Self {
            cooldown_time,
            original,
        })
    }
}
