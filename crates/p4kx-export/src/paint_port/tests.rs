// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutError;

use super::VehiclePorts;

#[test]
fn typed_missing_vehicle_files_remain_represented_source_facts() {
    let path = "synthetic/vehicle.xml";
    let facts = VehiclePorts::of(
        None,
        Some(path),
        &[LoadoutError::MissingVehicleImplementation {
            path: path.to_owned(),
        }],
    );
    assert!(
        facts.unreadable.is_none(),
        "a represented absent target is not a failed byte read"
    );
}

#[test]
fn unreadable_vehicle_files_are_not_classified_from_their_display_text() {
    let path = "synthetic/vehicle.xml";
    let facts = VehiclePorts::of(
        None,
        Some(path),
        &[LoadoutError::VehicleImplementation {
            path: path.to_owned(),
            reason: "there is no such file".to_owned(),
        }],
    );
    assert!(
        facts.unreadable.is_some(),
        "an actual unreadable result remains blocking even if its text resembles NotFound"
    );
}
