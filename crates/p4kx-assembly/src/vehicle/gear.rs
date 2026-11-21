// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_dataforge::{RecordTree, Value};

use super::LandingGear;
use crate::gear_placement_source::GearDeclaration;
use crate::read::{float, indexed_struct_items, struct_field, text};

/// Reads the gears of a `VehicleLandingGearSystem` Record.
pub(crate) fn gears(tree: &RecordTree<'_>) -> Arc<[LandingGear]> {
    indexed_struct_items(tree.root(), "gears")
        .map(|(ordinal, gear)| LandingGear {
            source: GearDeclaration {
                record: tree.record_index(),
                ordinal,
                authored_bone: matches!(
                    gear.field("bone"),
                    Some(Value::String(_) | Value::Locale(_) | Value::Enum(_))
                ),
            },
            bone: text(gear, "bone").to_owned(),
            geometry: struct_field(gear, "geometry")
                .map_or("", |geometry| text(geometry, "path"))
                .to_owned(),
            mass: float(gear, "mass"),
        })
        .collect()
}
