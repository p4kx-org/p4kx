// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use crate::CatalogError;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_text_contract::GameTextRef;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::SourceArenaOutput;

const SOURCE_TYPE: &str = "InfractionParameters";

/// One original default or override occurrence of the declared penalty parameters.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(super) struct InfractionParameters<'source> {
    source_type: SourcedValue<&'source str>,
    #[serde(flatten)]
    flags: InfractionFlags,
    grace_allowance: SourcedValue<i32>,
    grace_allowance_cooldown: SourcedValue<ResourceFloat32>,
    grace_period: SourcedValue<ResourceFloat32>,
    grace_cooloff_scale: SourcedValue<ResourceFloat32>,
    #[schemars(extend("x-gameText" = true))]
    grace_warnings: SourcedValue<Vec<SourcedValue<GameTextRef>>>,
    escalated_payment_fine_multiplier: SourcedValue<ResourceFloat32>,
    early_payment_period: SourcedValue<ResourceFloat32>,
    lifetime: SourcedValue<ResourceFloat32>,
    cool_off_time: SourcedValue<ResourceFloat32>,
    press_charges_notification_time: SourcedValue<ResourceFloat32>,
    remove_time_seconds: SourcedValue<ResourceFloat32>,
    felony_merits: SourcedValue<i32>,
    authored_fields: SourceArenaOutput<'source>,
}

impl<'source> InfractionParameters<'source> {
    pub(super) fn read_field(
        fields: Observed<'_, '_, '_, 'source>,
        name: &str,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<SourcedValue<Self>, CatalogError> {
        fields.declared_class_to(name, SOURCE_TYPE, notes, |owned, notes| {
            Self::read(owned, projection, notes)
        })
    }

    fn read(
        fields: Observed<'_, '_, '_, 'source>,
        projection: &CatalogProjection<'_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let flags = InfractionFlags::read(fields, notes)?;
        let grace_allowance = fields.int32("graceAllowance", notes)?;
        let grace_allowance_cooldown = fields.single("graceAllowanceCooldown", notes)?;
        let grace_period = fields.single("gracePeriod", notes)?;
        let grace_cooloff_scale = fields.single("graceCooloffScale", notes)?;
        let grace_warnings = fields.protected_locale_array("graceWarnings", projection, notes)?;
        let escalated_payment_fine_multiplier =
            fields.single("escalatedPaymentFineMultiplier", notes)?;
        let early_payment_period = fields.single("earlyPaymentPeriod", notes)?;
        let lifetime = fields.single("lifetime", notes)?;
        let cool_off_time = fields.single("coolOffTime", notes)?;
        let press_charges_notification_time =
            fields.single("pressChargesNotificationTime", notes)?;
        let remove_time_seconds = fields.single("removeTimeSeconds", notes)?;
        let felony_merits = fields.int32("felonyMerits", notes)?;
        let authored_fields =
            fields.authored_members_except(flags.projected().into_iter().chain([
                ("graceAllowance", grace_allowance.state()),
                ("graceAllowanceCooldown", grace_allowance_cooldown.state()),
                ("gracePeriod", grace_period.state()),
                ("graceCooloffScale", grace_cooloff_scale.state()),
                ("graceWarnings", grace_warnings.state()),
                (
                    "escalatedPaymentFineMultiplier",
                    escalated_payment_fine_multiplier.state(),
                ),
                ("earlyPaymentPeriod", early_payment_period.state()),
                ("lifetime", lifetime.state()),
                ("coolOffTime", cool_off_time.state()),
                (
                    "pressChargesNotificationTime",
                    press_charges_notification_time.state(),
                ),
                ("removeTimeSeconds", remove_time_seconds.state()),
                ("felonyMerits", felony_merits.state()),
            ]))?;
        Ok(Self {
            source_type: fields.source_type_value()?,
            flags,
            grace_allowance,
            grace_allowance_cooldown,
            grace_period,
            grace_cooloff_scale,
            grace_warnings,
            escalated_payment_fine_multiplier,
            early_payment_period,
            lifetime,
            cool_off_time,
            press_charges_notification_time,
            remove_time_seconds,
            felony_merits,
            authored_fields,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct InfractionFlags {
    is_felony: SourcedValue<i32>,
    display_grace_time: SourcedValue<i32>,
    ignore_if_against_party_member: SourcedValue<i32>,
    hide_crime_notification: SourcedValue<i32>,
    hide_crime_in_journal: SourcedValue<i32>,
}

impl InfractionFlags {
    fn read(
        fields: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            is_felony: fields.int32("isFelony", notes)?,
            display_grace_time: fields.int32("displayGraceTime", notes)?,
            ignore_if_against_party_member: fields.int32("ignoreIfAgainstPartyMember", notes)?,
            hide_crime_notification: fields.int32("hideCrimeNotification", notes)?,
            hide_crime_in_journal: fields.int32("hideCrimeInJournal", notes)?,
        })
    }

    const fn projected(&self) -> [(&'static str, SourceValueState); 5] {
        [
            ("isFelony", self.is_felony.state()),
            ("displayGraceTime", self.display_grace_time.state()),
            (
                "ignoreIfAgainstPartyMember",
                self.ignore_if_against_party_member.state(),
            ),
            (
                "hideCrimeNotification",
                self.hide_crime_notification.state(),
            ),
            ("hideCrimeInJournal", self.hide_crime_in_journal.state()),
        ]
    }
}
