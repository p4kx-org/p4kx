// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{self, Component, ComponentKey};
use super::number::Number;
use super::record_link::RecordLink;
use crate::catalog_queries::equipment::vehicle_armor::{
    ArmorDamageInfo, ArmorDeflection, ArmorHealthCurve, ArmorPenetrationResistance, CurvePoint,
    HealthLink, VehicleArmor, VehicleArmorDamage,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::StagedCatalogTable;
use crate::guid_text::CatalogGuid;
use crate::resource_float::ResourceFloat32;
use crate::source_value_catalog::{
    OriginalInstanceLocation, SourceWeakReference, SourceWeakTarget,
};

pub(super) const PATH: &str = "configurations.vehicleArmor";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_vehicle_armor(CatalogRecordSelection::All, cancel, |snapshot| {
        component::stage_selected(PATH, &snapshot, scratch, cancel, ArmorConfiguration::read)
    })?
}

type Quantity = Cell<Number<'static>>;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArmorConfiguration<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    source_type: Cell<&'read str>,
    signal_infrared: Quantity,
    signal_electromagnetic: Quantity,
    signal_cross_section: Quantity,
    damage_multiplier: Cell<Damage<'read>>,
    armor_penetration_resistance: Cell<PenetrationResistance<'read>>,
    armor_deflection: Cell<DeflectionProfile<'read>>,
}

impl<'read> ArmorConfiguration<'read> {
    fn read(original: &'read VehicleArmor<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(original.owner(), PATH)?;
        Ok(Self {
            owner: original.owner().into(),
            source_type: source_type(original.source_type())?,
            signal_infrared: quantity(original.signal_infrared())?,
            signal_electromagnetic: quantity(original.signal_electromagnetic())?,
            signal_cross_section: quantity(original.signal_cross_section())?,
            damage_multiplier: completeness::cell(PATH, original.damage_multiplier())?
                .try_map(Damage::read)?,
            armor_penetration_resistance: completeness::cell(
                PATH,
                original.armor_penetration_resistance(),
            )?
            .try_map(PenetrationResistance::read)?,
            armor_deflection: completeness::cell(PATH, original.armor_deflection())?
                .try_map(DeflectionProfile::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum Damage<'read> {
    Base {
        source_type: Cell<&'read str>,
    },
    Info {
        #[serde(flatten)]
        value: Box<DamageInfo<'read>>,
    },
    Parameters {
        source_type: Cell<&'read str>,
        damage_macro: Cell<MacroLink<'read>>,
        damage_total: Quantity,
    },
    Unknown {
        source_type: Cell<&'read str>,
    },
}

impl<'read> Damage<'read> {
    fn read(original: &'read VehicleArmorDamage<'_>) -> Result<Self, ReducedError> {
        Ok(match original {
            VehicleArmorDamage::Base { source } => Self::Base {
                source_type: source_type(source.source_type())?,
            },
            VehicleArmorDamage::Info { value } => Self::Info {
                value: Box::new(DamageInfo::read(value)?),
            },
            VehicleArmorDamage::Parameters { source, value } => Self::Parameters {
                source_type: source_type(source.source_type())?,
                damage_macro: completeness::cell(PATH, value.damage_macro())?
                    .try_map(MacroLink::read)?,
                damage_total: quantity(value.damage_total())?,
            },
            VehicleArmorDamage::Unknown { source } => Self::Unknown {
                source_type: source_type(source.source_type())?,
            },
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DamageInfo<'read> {
    source_type: Cell<&'read str>,
    physical: Quantity,
    energy: Quantity,
    distortion: Quantity,
    thermal: Quantity,
    biochemical: Quantity,
    stun: Quantity,
}

impl<'read> DamageInfo<'read> {
    fn read(original: &'read ArmorDamageInfo<'_>) -> Result<Self, ReducedError> {
        let channels = original.channels();
        Ok(Self {
            source_type: source_type(original.source().source_type())?,
            physical: quantity(channels.physical())?,
            energy: quantity(channels.energy())?,
            distortion: quantity(channels.distortion())?,
            thermal: quantity(channels.thermal())?,
            biochemical: quantity(channels.biochemical())?,
            stun: quantity(channels.stun())?,
        })
    }
}

#[derive(Serialize)]
#[serde(tag = "type", content = "value", rename_all = "camelCase")]
enum MacroLink<'read> {
    RecordReference(RecordLink<'read>),
    WeakReference(WeakLink<'read>),
}

impl<'read> MacroLink<'read> {
    fn read(original: &'read HealthLink<'_>) -> Result<Self, ReducedError> {
        Ok(match original {
            HealthLink::RecordReference { value } => {
                Self::RecordReference(RecordLink::from_source(PATH, value)?)
            }
            HealthLink::WeakReference { value } => Self::WeakReference(WeakLink::read(value)?),
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WeakLink<'read> {
    record_guid: Cell<&'read CatalogGuid>,
    record_path: Cell<&'read str>,
    target_record_index: Cell<&'read u32>,
    target: WeakTarget<'read>,
}

impl<'read> WeakLink<'read> {
    fn read(original: &'read SourceWeakReference<'_>) -> Result<Self, ReducedError> {
        let target = match original.target() {
            SourceWeakTarget::Record { source_type: kind } => WeakTarget::Record {
                source_type: source_type(kind)?,
            },
            SourceWeakTarget::Instance {
                source_type: kind,
                location,
            } => WeakTarget::Instance {
                source_type: source_type(kind)?,
                location: completeness::cell(PATH, location)?,
            },
        };
        Ok(Self {
            record_guid: completeness::cell(PATH, original.record_guid())?,
            record_path: completeness::cell(PATH, original.record_path())?.map(|value| *value),
            target_record_index: completeness::cell(PATH, original.target_record_index())?,
            target,
        })
    }
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum WeakTarget<'read> {
    Record {
        source_type: Cell<&'read str>,
    },
    Instance {
        source_type: Cell<&'read str>,
        location: Cell<&'read OriginalInstanceLocation<'read>>,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PenetrationResistance<'read> {
    source_type: Cell<&'read str>,
    base_penetration_reduction: Quantity,
    penetration_absorption_for_type: Cell<DamageInfo<'read>>,
}

impl<'read> PenetrationResistance<'read> {
    fn read(original: &'read ArmorPenetrationResistance<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            source_type: source_type(original.source_type())?,
            base_penetration_reduction: quantity(original.base_penetration_reduction())?,
            penetration_absorption_for_type: completeness::cell(
                PATH,
                original.penetration_absorption_for_type(),
            )?
            .try_map(DamageInfo::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct DeflectionProfile<'read> {
    source_type: Cell<&'read str>,
    deflection_value: Cell<DamageInfo<'read>>,
    health_curve: Cell<HealthCurve<'read>>,
}

impl<'read> DeflectionProfile<'read> {
    fn read(original: &'read ArmorDeflection<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            source_type: source_type(original.source_type())?,
            deflection_value: completeness::cell(PATH, original.deflection_value())?
                .try_map(DamageInfo::read)?,
            health_curve: completeness::cell(PATH, original.health_curve())?
                .try_map(HealthCurve::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HealthCurve<'read> {
    source_type: Cell<&'read str>,
    use_lut: Cell<&'read bool>,
    points: Cell<Vec<Cell<Point>>>,
}

impl<'read> HealthCurve<'read> {
    fn read(original: &'read ArmorHealthCurve<'_>) -> Result<Self, ReducedError> {
        let curve = original.curve();
        Ok(Self {
            source_type: source_type(original.source_type())?,
            use_lut: completeness::cell(PATH, &curve.use_lut)?,
            points: completeness::list(PATH, &curve.points, Point::read)?,
        })
    }
}

#[derive(Serialize)]
struct Point {
    x: Quantity,
    y: Quantity,
}

impl Point {
    fn read(original: &CurvePoint) -> Result<Self, ReducedError> {
        Ok(Self {
            x: quantity(&original.x)?,
            y: quantity(&original.y)?,
        })
    }
}

fn quantity(original: &SourcedValue<ResourceFloat32>) -> Result<Quantity, ReducedError> {
    Ok(completeness::cell(PATH, original)?.map(Number::from))
}

fn source_type<'read>(
    original: &'read SourcedValue<&str>,
) -> Result<Cell<&'read str>, ReducedError> {
    Ok(completeness::cell(PATH, original)?.map(|value| *value))
}
