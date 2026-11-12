// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::SourcedValue;
use schemars::JsonSchema;
use serde::Serialize;

use super::super::authored_fields::Observed;
use super::super::catalog_unit_source::UnitSource;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "MedBedSurgeryNames";
const INJURY_SOURCE: &str = "MedBedSurgeryNamesInjurySeverity";

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct MedBedSurgeryNames<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(rename = "headInjuries")]
    head: SourcedValue<MedBedInjuryNames<'source>>,
    #[serde(rename = "torsoInjuries")]
    torso: SourcedValue<MedBedInjuryNames<'source>>,
    #[serde(rename = "leftArmInjuries")]
    left_arm: SourcedValue<MedBedInjuryNames<'source>>,
    #[serde(rename = "rightArmInjuries")]
    right_arm: SourcedValue<MedBedInjuryNames<'source>>,
    #[serde(rename = "leftLegInjuries")]
    left_leg: SourcedValue<MedBedInjuryNames<'source>>,
    #[serde(rename = "rightLegInjuries")]
    right_leg: SourcedValue<MedBedInjuryNames<'source>>,
}

impl<'source> MedBedSurgeryNames<'source> {
    pub(super) fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let field = |name, notes: &mut Vec<FieldError>| {
            source.declared_class_to(name, INJURY_SOURCE, notes, |source, notes| {
                MedBedInjuryNames::read(source, projection, notes)
            })
        };
        let head = field("headInjuries", notes)?;
        let torso = field("torsoInjuries", notes)?;
        let left_arm = field("leftArmInjuries", notes)?;
        let right_arm = field("rightArmInjuries", notes)?;
        let left_leg = field("leftLegInjuries", notes)?;
        let right_leg = field("rightLegInjuries", notes)?;
        let source = UnitSource::read(
            source,
            [
                ("headInjuries", head.state()),
                ("torsoInjuries", torso.state()),
                ("leftArmInjuries", left_arm.state()),
                ("rightArmInjuries", right_arm.state()),
                ("leftLegInjuries", left_leg.state()),
                ("rightLegInjuries", right_leg.state()),
            ],
        )?;
        Ok(Self {
            source,
            head,
            torso,
            left_arm,
            right_arm,
            left_leg,
            right_leg,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct MedBedInjuryNames<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[schemars(extend("x-gameText" = true))]
    #[serde(rename = "injuryName")]
    injury: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    #[serde(rename = "majorInjuryName")]
    major: SourcedValue<GameTextRef>,
    #[schemars(extend("x-gameText" = true))]
    #[serde(rename = "deadlyInjuryName")]
    deadly: SourcedValue<GameTextRef>,
}

impl<'source> MedBedInjuryNames<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let injury = source.protected_locale("injuryName", projection, notes)?;
        let major = source.protected_locale("majorInjuryName", projection, notes)?;
        let deadly = source.protected_locale("deadlyInjuryName", projection, notes)?;
        let source = UnitSource::read(
            source,
            [
                ("injuryName", injury.state()),
                ("majorInjuryName", major.state()),
                ("deadlyInjuryName", deadly.state()),
            ],
        )?;
        Ok(Self {
            source,
            injury,
            major,
            deadly,
        })
    }
}
