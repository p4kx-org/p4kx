// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_dataforge::{DataForge, StructValue, Value};
use p4kx_extraction::SourceValueState;

use crate::read::{narrow, value_float};

const CONFIG: &str = "UIHoloVehicle_Config";
const DEFAULT_FOV: f32 = 10.0;
const DEFAULT_VIEW_ANGLE: f32 = 60.0;
const DEFAULT_DISTANCE_SCALER: f32 = 1.5;

/// The camera of the vehicle hologram the game's `UIHoloVehicle_Config` Record sets: the field
/// of view, the angle the vehicle is turned in and how far the camera stands from it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HologramCamera {
    fov_degrees: f32,
    view_angle_degrees: f32,
    distance_scaler: f32,
}

impl Default for HologramCamera {
    /// Returns the values the Record has in the current game: a field of view of 10 degrees, a
    /// view angle of 60 degrees and a distance scaler of 1.5.
    fn default() -> Self {
        Self {
            fov_degrees: DEFAULT_FOV,
            view_angle_degrees: DEFAULT_VIEW_ANGLE,
            distance_scaler: DEFAULT_DISTANCE_SCALER,
        }
    }
}

impl HologramCamera {
    /// Returns the camera of a field of view in degrees, a view angle in degrees and a distance
    /// scaler.
    #[must_use]
    pub const fn new(fov_degrees: f32, view_angle_degrees: f32, distance_scaler: f32) -> Self {
        Self {
            fov_degrees,
            view_angle_degrees,
            distance_scaler,
        }
    }

    /// Reads the camera from the `UIHoloVehicle_Config` Record of `forge`, a field it lacks
    /// keeping the value of [`HologramCamera::default`], or returns `None` when the DataForge
    /// has no such Record; with several the one with the smallest name is read.
    #[must_use]
    pub fn read(forge: &DataForge<'_>) -> Option<Self> {
        Self::read_observed(forge).camera()
    }

    /// Reads the same selected camera while retaining the original field read outcomes.
    #[must_use]
    pub fn read_observed(forge: &DataForge<'_>) -> ObservedHologramCamera {
        let record = forge
            .records()
            .filter(|record| record.type_name().is_ok_and(|name| name == CONFIG))
            .min_by_key(|record| record.name().unwrap_or_default());
        let Some(record) = record else {
            return ObservedHologramCamera::without_record();
        };
        let record_index = record.index();
        let Ok(tree) = record.tree() else {
            return ObservedHologramCamera::unreadable(record_index);
        };
        let root = tree.root();
        let default = Self::default();
        let fov = setting(root, HologramCameraField::FieldOfView, default.fov_degrees);
        let angle = setting(
            root,
            HologramCameraField::ViewAngle,
            default.view_angle_degrees,
        );
        let distance = setting(
            root,
            HologramCameraField::DistanceScaler,
            default.distance_scaler,
        );
        ObservedHologramCamera {
            record_index: Some(record_index),
            camera: Some(Self::new(fov.value, angle.value, distance.value)),
            fov: fov.state,
            angle: angle.state,
            distance: distance.state,
        }
    }

    /// Returns the vertical field of view in degrees.
    #[must_use]
    pub const fn fov_degrees(&self) -> f32 {
        self.fov_degrees
    }

    /// Returns the angle in degrees the vehicle is turned in, about its vertical axis.
    #[must_use]
    pub const fn view_angle_degrees(&self) -> f32 {
        self.view_angle_degrees
    }

    /// Returns the factor on the distance at which the vehicle just fills the view.
    #[must_use]
    pub const fn distance_scaler(&self) -> f32 {
        self.distance_scaler
    }
}

/// The three authored hologram camera settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum HologramCameraField {
    /// The vertical field of view.
    FieldOfView,
    /// The horizontal angle.
    ViewAngle,
    /// The factor applied to the camera distance.
    DistanceScaler,
}

impl HologramCameraField {
    /// Returns the original field of the selected camera Record.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::FieldOfView => "cameraFOV",
            Self::ViewAngle => "ownDefaultViewAngle",
            Self::DistanceScaler => "ownCameraDistanceScaler",
        }
    }
}

/// The actual selected camera Record and original read state of each field.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObservedHologramCamera {
    record_index: Option<u32>,
    camera: Option<HologramCamera>,
    fov: SourceValueState,
    angle: SourceValueState,
    distance: SourceValueState,
}

impl ObservedHologramCamera {
    const fn without_record() -> Self {
        Self {
            record_index: None,
            camera: None,
            fov: SourceValueState::Absent,
            angle: SourceValueState::Absent,
            distance: SourceValueState::Absent,
        }
    }

    const fn unreadable(record_index: u32) -> Self {
        Self {
            record_index: Some(record_index),
            camera: None,
            fov: SourceValueState::Unreadable,
            angle: SourceValueState::Unreadable,
            distance: SourceValueState::Unreadable,
        }
    }

    /// Returns the selected camera, with defaults for individual unreadable fields.
    #[must_use]
    pub const fn camera(&self) -> Option<HologramCamera> {
        self.camera
    }

    /// Returns the physical Record position selected before reading its fields.
    #[must_use]
    pub const fn record_index(&self) -> Option<u32> {
        self.record_index
    }

    /// Returns the original state of the requested setting before applying defaults.
    #[must_use]
    pub const fn field_state(&self, field: HologramCameraField) -> SourceValueState {
        match field {
            HologramCameraField::FieldOfView => self.fov,
            HologramCameraField::ViewAngle => self.angle,
            HologramCameraField::DistanceScaler => self.distance,
        }
    }
}

struct Setting {
    value: f32,
    state: SourceValueState,
}

fn setting(root: &StructValue<'_>, field: HologramCameraField, fallback: f32) -> Setting {
    let value = root.field(field.name());
    let (value, state) = match value {
        Some(Value::Null) => (fallback, SourceValueState::Null),
        Some(Value::Error(_)) => (fallback, SourceValueState::Unreadable),
        Some(value) => value_float(value).map_or((fallback, SourceValueState::Invalid), |number| {
            (narrow(number), SourceValueState::Present)
        }),
        None => (fallback, SourceValueState::Absent),
    };
    Setting { value, state }
}
