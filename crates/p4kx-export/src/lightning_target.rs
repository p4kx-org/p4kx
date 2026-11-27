// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::CatalogError;
use crate::catalog_v3::authored_fields::Observed;
use crate::catalog_v3::catalog_unit_source::UnitSource;
use crate::catalog_v3::projectile_range::ProjectileRange;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const SOURCE_TYPE: &str = "LightningTargetMode";
const INTERNAL_PLANET: &str = "LightningTargetMode_InternalPlanet";
const RANGE: &str = "Range";
const ENABLE: &str = "enable";
const MINIMUM_INTERVAL: &str = "minimumInterval";
const REFERENCE_INTERVAL: &str = "referenceInterval";
const LENGTH: &str = "length";
const SCALE_MULTIPLIER: &str = "scaleMultiplier";
const INTENSITY: &str = "intensity";
const DISTANCE_RANGE: &str = "distanceRange";

/// A concrete target rule, its inherited flag and its original authored members.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LightningTargetMode<'source> {
    source_type: SourcedValue<&'source str>,
    enable: SourcedValue<bool>,
    payload: SourcedValue<LightningTargetPayload<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> LightningTargetMode<'source> {
    /// Borrows the exact concrete target type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the inherited source flag.
    #[must_use]
    pub const fn enable(&self) -> &SourcedValue<bool> {
        &self.enable
    }

    /// Borrows the typed payload selected by the original concrete type.
    #[must_use]
    pub const fn payload(&self) -> &SourcedValue<LightningTargetPayload<'source>> {
        &self.payload
    }

    /// Borrows original members outside the typed target fields.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    pub(super) fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let source_type = fields.source_type_value()?;
        let enable = fields.boolean(ENABLE, notes)?;
        let payload = source_type
            .clone()
            .try_map(|source_type| LightningTargetPayload::read(source_type, fields, notes))?;
        let typed_fields = match payload.value() {
            Some(LightningTargetPayload::InternalPlanet(settings)) => Some(settings.projected()),
            _ => None,
        };
        let projected =
            std::iter::once((ENABLE, enable.state())).chain(typed_fields.into_iter().flatten());
        let additional_fields = fields.authored_members_except(projected)?;
        Ok(Self {
            source_type,
            enable,
            payload,
            additional_fields,
        })
    }
}

/// The original concrete target type's recognised payload.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(
    tag = "kind",
    content = "parameters",
    rename_all = "camelCase",
    deny_unknown_fields
)]
#[non_exhaustive]
pub enum LightningTargetPayload<'source> {
    /// Authored parameters of the original internal-planet rule.
    InternalPlanet(Box<InternalPlanetLightning<'source>>),
    /// Another original type whose complete members remain available on the rule.
    Other,
}

impl<'source> LightningTargetPayload<'source> {
    fn read(
        source_type: &str,
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        if source_type == INTERNAL_PLANET {
            let parameters = InternalPlanetLightning::read(fields, notes)?;
            Ok(Self::InternalPlanet(Box::new(parameters)))
        } else {
            Ok(Self::Other)
        }
    }
}

/// The six original parameters of an internal-planet lightning rule.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
#[non_exhaustive]
pub struct InternalPlanetLightning<'source> {
    /// Original minimum-interval scalar.
    pub minimum_interval: SourcedValue<ResourceFloat32>,
    /// Original reference-interval scalar.
    pub reference_interval: SourcedValue<ResourceFloat32>,
    /// Original length range.
    pub length: SourcedValue<LightningRange<'source>>,
    /// Original scale multiplier.
    pub scale_multiplier: SourcedValue<ResourceFloat32>,
    /// Original intensity scalar.
    pub intensity: SourcedValue<ResourceFloat32>,
    /// Original distance range.
    pub distance_range: SourcedValue<LightningRange<'source>>,
}

impl<'source> InternalPlanetLightning<'source> {
    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            minimum_interval: fields.single(MINIMUM_INTERVAL, notes)?,
            reference_interval: fields.single(REFERENCE_INTERVAL, notes)?,
            length: fields.declared_class_to(LENGTH, RANGE, notes, LightningRange::read)?,
            scale_multiplier: fields.single(SCALE_MULTIPLIER, notes)?,
            intensity: fields.single(INTENSITY, notes)?,
            distance_range: fields.declared_class_to(
                DISTANCE_RANGE,
                RANGE,
                notes,
                LightningRange::read,
            )?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 6] {
        [
            (MINIMUM_INTERVAL, self.minimum_interval.state()),
            (REFERENCE_INTERVAL, self.reference_interval.state()),
            (LENGTH, self.length.state()),
            (SCALE_MULTIPLIER, self.scale_multiplier.state()),
            (INTENSITY, self.intensity.state()),
            (DISTANCE_RANGE, self.distance_range.state()),
        ]
    }
}

/// An original Range with its separately sourced endpoints.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct LightningRange<'source> {
    #[serde(flatten)]
    endpoints: ProjectileRange,
    #[serde(flatten)]
    source: UnitSource<'source>,
}

impl<'source> LightningRange<'source> {
    /// Borrows the original minimum endpoint.
    #[must_use]
    pub const fn minimum(&self) -> &SourcedValue<ResourceFloat32> {
        self.endpoints.minimum()
    }

    /// Borrows the original maximum endpoint.
    #[must_use]
    pub const fn maximum(&self) -> &SourcedValue<ResourceFloat32> {
        self.endpoints.maximum()
    }

    /// Borrows the exact concrete type and additional original members.
    #[must_use]
    pub const fn source(&self) -> &UnitSource<'source> {
        &self.source
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let endpoints = ProjectileRange::read(fields, notes)?;
        let source = UnitSource::read(fields, endpoints.projected())?;
        Ok(Self { endpoints, source })
    }
}
