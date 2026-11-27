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
use crate::catalog_v3::catalog_item_components::source_getters;
use crate::resource_float::ResourceFloat32;
use crate::schema_fields::every_field_required;
use crate::source_fields::FieldError;
use crate::source_value_catalog::{SourceRecordReference, SourceWeakReference};

pub(super) const SOURCE_TYPE: &str = "SCItemEMPParams";
const STATE: &str = "ItemResourceState";
const PARTICLE: &str = "GlobalResourceParticle";
const TAG: &str = "Tag";

/// The stored configuration of one physical EMP component.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct Emp<'source> {
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
    source_getters! {
        /// Borrows the original declaration and additional fields.
        source: UnitSource<'source>,
        /// Borrows the authored EMP timing, radius and damage settings.
        parameters: EmpParameters,
        /// Borrows the authored stage states, particles and tags.
        stages: EmpStages<'source>,
        /// Borrows the authored trigger declarations.
        triggers: EmpTriggers<'source>,
    }

    /// Borrows the original Item Record and physical Components slot.
    #[must_use]
    pub const fn owner(&self) -> &ComponentOwner {
        &self.owner
    }

    /// Borrows the original distortion damage.
    #[must_use]
    pub const fn distortion_damage(&self) -> &SourcedValue<ResourceFloat32> {
        &self.parameters.distortion_damage
    }

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

/// Authored EMP timing, radius and force settings.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EmpParameters {
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
    source_getters! {
        /// Borrows the authored charge time.
        charge_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored unleash time.
        unleash_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored cooldown time.
        cooldown_time: SourcedValue<ResourceFloat32>,
        /// Borrows the authored EMP radius.
        emp_radius: SourcedValue<ResourceFloat32>,
        /// Borrows the authored min EMP radius.
        min_emp_radius: SourcedValue<ResourceFloat32>,
        /// Borrows the authored phys radius.
        phys_radius: SourcedValue<ResourceFloat32>,
        /// Borrows the authored min phys radius.
        min_phys_radius: SourcedValue<ResourceFloat32>,
        /// Borrows the authored distortion damage.
        distortion_damage: SourcedValue<ResourceFloat32>,
        /// Borrows the authored pressure.
        pressure: SourcedValue<ResourceFloat32>,
    }

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

/// Authored EMP stage states, particles and tags.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EmpStages<'source> {
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
    source_getters! {
        /// Borrows the authored idle state.
        idle_state: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Borrows the authored charging state.
        charging_state: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Borrows the authored charged state.
        charged_state: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Borrows the authored releasing state.
        releasing_state: SourcedValue<Box<SourceWeakReference<'source>>>,
        /// Borrows the authored charging particle.
        charging_particle: SourcedValue<EmpParticle<'source>>,
        /// Borrows the authored charged particle.
        charged_particle: SourcedValue<EmpParticle<'source>>,
        /// Borrows the authored charging tag.
        charging_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// Borrows the authored charged tag.
        charged_tag: SourcedValue<Box<SourceRecordReference<'source>>>,
    }

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

/// The source class and path of an EMP particle.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(transform = every_field_required)]
pub struct EmpParticle<'source> {
    #[serde(flatten)]
    source: UnitSource<'source>,
    path: SourcedValue<&'source str>,
}

impl<'source> EmpParticle<'source> {
    source_getters! {
        /// Borrows the original declaration and additional fields.
        source: UnitSource<'source>,
        /// Borrows the authored resource path.
        path: SourcedValue<&'source str>,
    }

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

/// Authored audio trigger references by EMP stage.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub struct EmpTriggers<'source> {
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
    source_getters! {
        /// Borrows the authored start charging.
        start_charging: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// Borrows the authored stop charging.
        stop_charging: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// Borrows the authored start charged.
        start_charged: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// Borrows the authored stop charged.
        stop_charged: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// Borrows the authored start unleash.
        start_unleash: SourcedValue<Box<SourceRecordReference<'source>>>,
        /// Borrows the authored stop unleash.
        stop_unleash: SourcedValue<Box<SourceRecordReference<'source>>>,
    }

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
