// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use super::{record_query, rows};

pub use crate::catalog_v3::catalog_entity_configurations::{
    ArmorDamageInfo, ArmorDamageSource, ArmorDeflection, ArmorHealthCurve,
    ArmorPenetrationResistance, CurvePoint, DamageChannels, DamageParameters, DegradationCurve,
    HealthLink, VehicleArmor, VehicleArmorDamage,
};

record_query::unlocalized!(
    rows::VehicleArmorRows,
    VehicleArmor,
    /// Original vehicle armor rows and their original evidence.
    VehicleArmorSnapshot,
    with_vehicle_armor,
    with_vehicle_armor_from
);
