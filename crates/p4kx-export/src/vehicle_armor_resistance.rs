// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::degradation_curve::DegradationCurve;
use super::vehicle_armor_damage::{self, ArmorDamageInfo};
use crate::CatalogError;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

pub(super) const PENETRATION: &str = "ArmorPenetrationResistanceParams";
pub(super) const DEFLECTION: &str = "ArmorDeflectionParams";

/// Original base and per-type penetration settings of a vehicle armour component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ArmorPenetrationResistance<'source> {
    source_type: SourcedValue<&'source str>,
    base_penetration_reduction: SourcedValue<ResourceFloat32>,
    penetration_absorption_for_type: SourcedValue<ArmorDamageInfo<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ArmorPenetrationResistance<'source> {
    /// Borrows the concrete authored type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the authored penetration reduction.
    #[must_use]
    pub const fn base_penetration_reduction(&self) -> &SourcedValue<ResourceFloat32> {
        &self.base_penetration_reduction
    }

    /// Borrows the original absorption values.
    #[must_use]
    pub const fn penetration_absorption_for_type(&self) -> &SourcedValue<ArmorDamageInfo<'source>> {
        &self.penetration_absorption_for_type
    }

    /// Borrows the remaining original members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let base_penetration_reduction = source.single("basePenetrationReduction", notes)?;
        let penetration_absorption_for_type = source.declared_class_to(
            "penetrationAbsorptionForType",
            vehicle_armor_damage::INFO,
            notes,
            ArmorDamageInfo::read,
        )?;
        let additional_fields = source.authored_members_except([
            (
                "basePenetrationReduction",
                base_penetration_reduction.state(),
            ),
            (
                "penetrationAbsorptionForType",
                penetration_absorption_for_type.state(),
            ),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            base_penetration_reduction,
            penetration_absorption_for_type,
            additional_fields,
        })
    }
}

/// The authored deflection values and inline health curve.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ArmorDeflection<'source> {
    source_type: SourcedValue<&'source str>,
    deflection_value: SourcedValue<ArmorDamageInfo<'source>>,
    health_curve: SourcedValue<ArmorHealthCurve<'source>>,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ArmorDeflection<'source> {
    /// Borrows the concrete authored type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the original deflection values.
    #[must_use]
    pub const fn deflection_value(&self) -> &SourcedValue<ArmorDamageInfo<'source>> {
        &self.deflection_value
    }

    /// Borrows the authored health curve.
    #[must_use]
    pub const fn health_curve(&self) -> &SourcedValue<ArmorHealthCurve<'source>> {
        &self.health_curve
    }

    /// Borrows the remaining original members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let deflection_value = source.declared_class_to(
            "deflectionValue",
            vehicle_armor_damage::INFO,
            notes,
            ArmorDamageInfo::read,
        )?;
        let health_curve = source.declared_class_to(
            "healthCurve",
            "BezierCurve",
            notes,
            ArmorHealthCurve::read,
        )?;
        let additional_fields = source.authored_members_except([
            ("deflectionValue", deflection_value.state()),
            ("healthCurve", health_curve.state()),
        ])?;
        Ok(Self {
            source_type: source.source_type_value()?,
            deflection_value,
            health_curve,
            additional_fields,
        })
    }
}

/// The original curve controls and ordered authored point slots.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct ArmorHealthCurve<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    curve: DegradationCurve,
    additional_fields: SourceArenaOutput<'source>,
}

impl<'source> ArmorHealthCurve<'source> {
    /// Borrows the concrete authored type.
    #[must_use]
    pub const fn source_type(&self) -> &SourcedValue<&'source str> {
        &self.source_type
    }

    /// Borrows the original curve controls and point slots.
    #[must_use]
    pub const fn curve(&self) -> &DegradationCurve {
        &self.curve
    }

    /// Borrows the remaining original members.
    #[must_use]
    pub const fn additional_fields(&self) -> &SourceArenaOutput<'source> {
        &self.additional_fields
    }

    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let curve = DegradationCurve::read(source, notes)?;
        let additional_fields = source.authored_members_except(curve.projected())?;
        Ok(Self {
            source_type: source.source_type_value()?,
            curve,
            additional_fields,
        })
    }
}
