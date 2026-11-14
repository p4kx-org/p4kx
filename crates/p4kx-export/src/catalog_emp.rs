// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{DataForge, SourceValueState, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use super::authored_fields::Observed;
use super::catalog_unit_source::UnitSource;
use super::interaction_source::{RecordLink, WeakLink};
use super::item_component_owner::ComponentOwner;
use crate::CatalogError;
use crate::catalog_record_reference::CatalogRecordReference;
use crate::catalog_table::CatalogTable;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;

pub(super) const SOURCE_TYPE: &str = "SCItemEMPParams";
const STATE: &str = "ItemResourceState";
const PARTICLE: &str = "GlobalResourceParticle";
const TAG: &str = "Tag";

/// The stored configuration of one physical EMP component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct Emp<'source> {
    #[serde(flatten)]
    owner: ComponentOwner,
    #[serde(flatten)]
    source: UnitSource<'source>,
    #[serde(flatten)]
    parameters: EmpParameters,
    #[serde(flatten)]
    stages: EmpStages<'source>,
    #[serde(flatten)]
    triggers: EmpTriggers<'source>,
}

impl<'source> Emp<'source> {
    pub(super) fn read(
        owner: ComponentOwner,
        observed: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let parameters = EmpParameters::read(observed, notes)?;
        let stages = EmpStages::read(observed, notes)?;
        let triggers = EmpTriggers::read(observed, notes)?;
        let projected = parameters
            .members()
            .into_iter()
            .chain(stages.members())
            .chain(triggers.members());
        Ok(Self {
            owner,
            source: UnitSource::read(observed, projected)?,
            parameters,
            stages,
            triggers,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EmpParameters {
    charge_time: SourcedValue<ResourceFloat32>,
    unleash_time: SourcedValue<ResourceFloat32>,
    cooldown_time: SourcedValue<ResourceFloat32>,
    emp_radius: SourcedValue<ResourceFloat32>,
    min_emp_radius: SourcedValue<ResourceFloat32>,
    phys_radius: SourcedValue<ResourceFloat32>,
    min_phys_radius: SourcedValue<ResourceFloat32>,
    distortion_damage: SourcedValue<ResourceFloat32>,
    pressure: SourcedValue<ResourceFloat32>,
}

impl EmpParameters {
    fn read(
        source: Observed<'_, '_, '_, '_>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            charge_time: source.single("chargeTime", notes)?,
            unleash_time: source.single("unleashTime", notes)?,
            cooldown_time: source.single("cooldownTime", notes)?,
            emp_radius: source.single("empRadius", notes)?,
            min_emp_radius: source.single("minEmpRadius", notes)?,
            phys_radius: source.single("physRadius", notes)?,
            min_phys_radius: source.single("minPhysRadius", notes)?,
            distortion_damage: source.single("distortionDamage", notes)?,
            pressure: source.single("pressure", notes)?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 9] {
        [
            ("chargeTime", self.charge_time.state()),
            ("unleashTime", self.unleash_time.state()),
            ("cooldownTime", self.cooldown_time.state()),
            ("empRadius", self.emp_radius.state()),
            ("minEmpRadius", self.min_emp_radius.state()),
            ("physRadius", self.phys_radius.state()),
            ("minPhysRadius", self.min_phys_radius.state()),
            ("distortionDamage", self.distortion_damage.state()),
            ("pressure", self.pressure.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EmpStages<'source> {
    idle_state: WeakLink<'source>,
    charging_state: WeakLink<'source>,
    charged_state: WeakLink<'source>,
    releasing_state: WeakLink<'source>,
    charging_particle: SourcedValue<EmpParticle<'source>>,
    charged_particle: SourcedValue<EmpParticle<'source>>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    charging_tag: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    charged_tag: RecordLink<'source>,
}

impl<'source> EmpStages<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            idle_state: source.weak_link_to("idleState", STATE, notes)?,
            charging_state: source.weak_link_to("chargingState", STATE, notes)?,
            charged_state: source.weak_link_to("chargedState", STATE, notes)?,
            releasing_state: source.weak_link_to("releasingState", STATE, notes)?,
            charging_particle: source.declared_class_to(
                "ChargingParticle",
                PARTICLE,
                notes,
                EmpParticle::read,
            )?,
            charged_particle: source.declared_class_to(
                "ChargedParticle",
                PARTICLE,
                notes,
                EmpParticle::read,
            )?,
            charging_tag: source.record_link_to("ChargingTag", TAG, notes)?,
            charged_tag: source.record_link_to("ChargedTag", TAG, notes)?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 8] {
        [
            ("idleState", self.idle_state.state()),
            ("chargingState", self.charging_state.state()),
            ("chargedState", self.charged_state.state()),
            ("releasingState", self.releasing_state.state()),
            ("ChargingParticle", self.charging_particle.state()),
            ("ChargedParticle", self.charged_particle.state()),
            ("ChargingTag", self.charging_tag.state()),
            ("ChargedTag", self.charged_tag.state()),
        ]
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
struct EmpParticle<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    path: SourcedValue<&'source str>,
}

impl<'source> EmpParticle<'source> {
    fn read(
        observed: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        let path = observed.borrowed_string("path", notes)?;
        Ok(Self {
            source: UnitSource::read(observed, [("path", path.state())])?,
            path,
        })
    }
}

#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
struct EmpTriggers<'source> {
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "startChargingTrigger")]
    start_charging: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "stopChargingTrigger")]
    stop_charging: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "startChargedTrigger")]
    start_charged: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "stopChargedTrigger")]
    stop_charged: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "startUnleashTrigger")]
    start_unleash: RecordLink<'source>,
    #[schemars(with = "SourcedValue<CatalogRecordReference>")]
    #[serde(rename = "stopUnleashTrigger")]
    stop_unleash: RecordLink<'source>,
}

impl<'source> EmpTriggers<'source> {
    fn read(
        source: Observed<'_, '_, '_, 'source>,
        notes: &mut Vec<FieldError>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            start_charging: source.record_link_to("StartChargingTrigger", TAG, notes)?,
            stop_charging: source.record_link_to("StopChargingTrigger", TAG, notes)?,
            start_charged: source.record_link_to("StartChargedTrigger", TAG, notes)?,
            stop_charged: source.record_link_to("StopChargedTrigger", TAG, notes)?,
            start_unleash: source.record_link_to("StartUnleashTrigger", TAG, notes)?,
            stop_unleash: source.record_link_to("StopUnleashTrigger", TAG, notes)?,
        })
    }

    const fn members(&self) -> [(&'static str, SourceValueState); 6] {
        [
            ("StartChargingTrigger", self.start_charging.state()),
            ("StopChargingTrigger", self.stop_charging.state()),
            ("StartChargedTrigger", self.start_charged.state()),
            ("StopChargedTrigger", self.stop_charged.state()),
            ("StartUnleashTrigger", self.start_unleash.state()),
            ("StopUnleashTrigger", self.stop_unleash.state()),
        ]
    }
}

pub(crate) fn table<'source>(
    forge: &DataForge<'_>,
    mut rows: Vec<Emp<'source>>,
) -> CatalogTable<Emp<'source>> {
    rows.sort_by_key(|row| row.owner.key());
    CatalogTable::new(forge, &[SOURCE_TYPE], rows)
}
