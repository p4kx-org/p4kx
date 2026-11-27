// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Fixture {
    #[serde(rename = "fixtureVersion")]
    pub(super) version: u32,
    pub(super) formula_version: u32,
    pub(super) catalog_schema_version: u32,
    pub(super) default_quality: u32,
    pub(super) tolerance: Tolerance,
    pub(super) units: Units,
    pub(super) property_mappings: Vec<PropertyMapping>,
    pub(super) cases: Vec<Case>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Units {
    pub(super) damage: String,
    pub(super) nominal_dps: String,
    pub(super) travel_distance: String,
    pub(super) shield: String,
    pub(super) rate: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Tolerance {
    pub(super) absolute: f64,
    pub(super) relative: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct PropertyMapping {
    pub(super) property_id: String,
    pub(super) target: Stat,
    pub(super) better_when: Direction,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(rename_all = "camelCase")]
pub(super) enum Stat {
    DamagePerProjectile,
    Rpm,
    MaxHealth,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum Direction {
    Higher,
    Lower,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Case {
    pub(super) id: String,
    pub(super) build: Build,
    pub(super) parts: Vec<Part>,
    pub(super) loadouts: Option<Loadouts>,
    pub(super) expected: Value,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Build {
    pub(super) branch: String,
    pub(super) version: String,
    pub(super) change_number: String,
    #[serde(rename = "buildId")]
    pub(super) id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Part {
    pub(super) id: String,
    pub(super) item_guid: String,
    pub(super) blueprint_guid: String,
    pub(super) tier_index: u32,
    pub(super) input: Input,
    pub(super) slots: Vec<Slot>,
    #[serde(default)]
    pub(super) qualities: Vec<Quality>,
}

#[derive(Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub(super) enum Input {
    SimpleProjectile {
        ammo_status: AmmoStatus,
        ammo_cost: u32,
        pellet_count: u32,
        launcher_damage_multiplier: Option<f64>,
        rpm: Option<f64>,
        damage_per_projectile: Option<BTreeMap<String, f64>>,
        speed_metres_per_second: Option<f64>,
        lifetime_seconds: Option<f64>,
    },
    ShieldGenerator {
        max_health: Option<f64>,
    },
    Beam {
        authored_per_second: BTreeMap<String, f64>,
        full_damage_range: f64,
        zero_damage_range: f64,
    },
    Charged {
        charge_seconds: f64,
    },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum AmmoStatus {
    Selected,
    Missing,
    Ambiguous,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Slot {
    pub(super) slot_path: Vec<Step>,
    pub(super) modifiers: Vec<Modifier>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Step {
    Mandatory,
    Optional { index: u32 },
    Option { index: u32 },
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Quality {
    pub(super) blueprint_guid: String,
    pub(super) tier_index: u32,
    pub(super) slot_path: Vec<Step>,
    #[serde(rename = "quality")]
    pub(super) score: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Modifier {
    pub(super) property_id: String,
    pub(super) value_kind: Kind,
    pub(super) range_type: RangeType,
    pub(super) band: Option<Segment>,
    #[serde(default)]
    pub(super) segments: Vec<Segment>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub(super) enum Kind {
    Multiplier,
    Additive,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum RangeType {
    Linear,
    Step,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Segment {
    pub(super) a: Option<i64>,
    pub(super) b: Option<i64>,
    pub(super) start: Option<f64>,
    pub(super) end: Option<f64>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Loadouts {
    pub(super) factory: Vec<Placement>,
    pub(super) crafted: Vec<Placement>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Placement {
    pub(super) part_id: String,
    pub(super) port_path: Vec<String>,
    pub(super) group: Option<Group>,
    pub(super) qualities: Option<Vec<Quality>>,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum Group {
    Pilot,
    Turret { id: String },
}

impl Group {
    pub(super) fn key(&self) -> String {
        match self {
            Self::Pilot => "pilot".into(),
            Self::Turret { id } => format!("turret:{id}"),
        }
    }
}
