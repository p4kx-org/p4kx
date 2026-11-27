// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

//! The camera of the vehicle hologram, read from the `UIHoloVehicle_Config` Record.

use crate::assembly_support::{TestResult, entries, fixture};
use p4kx_assembly::{HologramCamera, HologramCameraField};
use p4kx_extraction::{Archive, DataForge, DataForgeEntry, SourceValueState};
use p4kx_testkit::{
    DataForgeBuilder, TestRecord, TestStruct, TestType, TestValue, hologram_entries,
};

fn read(
    entries: Vec<p4kx_testkit::TestEntry>,
) -> Result<Option<HologramCamera>, Box<dyn std::error::Error>> {
    let fixture = fixture(entries)?;
    let archive = Archive::open(&fixture.path)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    Ok(HologramCamera::read(&forge))
}

#[test]
fn the_record_sets_the_field_of_view_the_view_angle_and_the_distance_scaler() -> TestResult {
    let camera = read(hologram_entries([14.0, 45.0, 2.5]))?;

    assert_eq!(
        camera,
        Some(HologramCamera::new(14.0, 45.0, 2.5)),
        "the three fields of the Record"
    );
    Ok(())
}

#[test]
fn a_database_without_the_record_has_no_camera() -> TestResult {
    assert_eq!(read(entries())?, None, "no UIHoloVehicle_Config");
    Ok(())
}

#[test]
fn the_default_camera_is_what_the_current_game_sets() {
    let camera = HologramCamera::default();

    assert_eq!(camera.fov_degrees().to_bits(), 10.0_f32.to_bits(), "fov");
    assert_eq!(
        camera.view_angle_degrees().to_bits(),
        60.0_f32.to_bits(),
        "angle"
    );
    assert_eq!(
        camera.distance_scaler().to_bits(),
        1.5_f32.to_bits(),
        "scaler"
    );
}

#[test]
fn the_selected_physical_camera_retains_independent_setting_outcomes() -> TestResult {
    const TYPE: &str = "UIHoloVehicle_Config";
    let bytes = DataForgeBuilder::new()
        .structure(
            TestStruct::new(TYPE)
                .field("cameraFOV", TestType::Single)
                .field("ownDefaultViewAngle", TestType::String),
        )
        .record(
            TestRecord::new(1, TYPE, "UIHoloVehicle_Config.Z", "synthetic/z.xml")
                .value("cameraFOV", TestValue::Single(99.0)),
        )
        .record(
            TestRecord::new(2, TYPE, "UIHoloVehicle_Config.A", "synthetic/a.xml")
                .value("cameraFOV", TestValue::Single(14.0))
                .value("ownDefaultViewAngle", TestValue::text("wrong source type")),
        )
        .build();
    let forge = DataForge::open(&bytes)?;
    let selected = forge
        .records_at_path("synthetic/a")
        .next()
        .ok_or("selected Record")?;
    let observed = HologramCamera::read_observed(&forge);
    assert_eq!(observed.record_index(), Some(selected.index()));
    assert_eq!(
        observed.camera(),
        Some(HologramCamera::new(14.0, 60.0, 1.5))
    );
    assert_eq!(
        observed.field_state(HologramCameraField::FieldOfView),
        SourceValueState::Present
    );
    assert_eq!(
        observed.field_state(HologramCameraField::ViewAngle),
        SourceValueState::Invalid
    );
    assert_eq!(
        observed.field_state(HologramCameraField::DistanceScaler),
        SourceValueState::Absent
    );
    assert_eq!(observed.camera(), HologramCamera::read(&forge));
    Ok(())
}
