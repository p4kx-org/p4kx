// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::ComponentKind;
use crate::CatalogError;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::component_sources::physical_component;
use crate::catalog_v3::item_component_owner::ComponentOwner;
use crate::export_field_inspection::ConfigurationInspection;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{
    SourceArenaOutput, SourceDataType, SourcePayload, SourceRecordReference,
};

const COMPONENT: &str = "SCItemRadarComponentParams";
pub(crate) const SHARED_RADAR_PARAMS: &str = "RadarSystemSharedParams";
pub(super) const KIND: ComponentKind = ComponentKind {
    source_type: COMPONENT,
    inspection: ConfigurationInspection::ItemRadar,
};

physical_component!(ItemRadarComponents, ItemRadar, KIND);

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

    /// Borrows the original resolved shared-parameters Reference when its target is compatible.
    #[must_use]
    pub fn shared_params(&self) -> Option<&SourceRecordReference<'source>> {
        let arena = self.original.additional_fields();
        let field = arena
            .roots()
            .iter()
            .find(|field| field.name().value() == Some(&"sharedParams"))?;
        let declaration = field.declaration();
        if !matches!(
            declaration.data_type().value(),
            Some(SourceDataType::Reference)
        ) || declaration.is_array().value() != Some(&false)
        {
            return None;
        }
        let SourcePayload::RecordReference { value } =
            arena.value(field.value())?.value().value()?
        else {
            return None;
        };
        if value.target_type().value() != Some(&SHARED_RADAR_PARAMS) {
            return None;
        }
        value.resolved_index()?;
        Some(value)
    }

    pub(super) fn shared_params_index(&self) -> Option<u32> {
        self.shared_params()?.resolved_index()
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

/// One physical Record defining original shared radar configuration parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct RadarSystemSharedParams<'source> {
    source: CanonicalRecordHeader<'source>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> RadarSystemSharedParams<'source> {
    /// Borrows the original physical definition header.
    #[must_use]
    pub const fn source(&self) -> &CanonicalRecordHeader<'source> {
        &self.source
    }

    /// Borrows the complete original owned parameter tree and its field provenance.
    #[must_use]
    pub const fn authored_fields(&self) -> &SourceArenaOutput<'source> {
        &self.authored_fields
    }

    pub(in crate::catalog_v3) fn read(
        _source_type: &'source str,
        source: CanonicalRecordHeader<'source>,
        fields: Observed<'_, '_, '_, 'source>,
        _notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source,
            authored_fields: fields.authored_members()?,
        })
    }
}
