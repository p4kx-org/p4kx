// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::{Transform, attachment_offset};

use super::{relative_position, relative_rotation};

/// An original transform attribute of a CDF bone attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum AttachmentPoseField {
    /// Translation from the bone in metres.
    RelativePosition,
    /// Rotation from the bone in WXYZ order.
    RelativeRotation,
    /// Translation in the owning model's space in metres.
    ModelPosition,
    /// Rotation in the owning model's space in WXYZ order.
    ModelRotation,
}

impl AttachmentPoseField {
    /// Returns the original CDF attribute name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::RelativePosition => "RelPosition",
            Self::RelativeRotation => "RelRotation",
            Self::ModelPosition => "Position",
            Self::ModelRotation => "Rotation",
        }
    }
}

/// Selected authored values. A malformed selected attribute has no decoded value.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct AttachmentPose {
    /// Selected translation in metres.
    pub position: Option<[f32; 3]>,
    /// Selected quaternion in WXYZ order.
    pub rotation_wxyz: Option<[f32; 4]>,
    selection: AttachmentPoseSelection,
}

impl AttachmentPose {
    /// Reads each component from its relative attribute when present or else its model attribute.
    ///
    /// With no model attributes, missing values default in the bone frame.
    #[must_use]
    pub fn read<'input>(
        mut attribute: impl FnMut(AttachmentPoseField) -> Option<&'input str>,
    ) -> Self {
        let rel_position = attribute(AttachmentPoseField::RelativePosition);
        let rel_rotation = attribute(AttachmentPoseField::RelativeRotation);
        let model_position = attribute(AttachmentPoseField::ModelPosition);
        let model_rotation = attribute(AttachmentPoseField::ModelRotation);
        let position = rel_position
            .map(|_| AttachmentPoseField::RelativePosition)
            .or_else(|| model_position.map(|_| AttachmentPoseField::ModelPosition));
        let rotation = rel_rotation
            .map(|_| AttachmentPoseField::RelativeRotation)
            .or_else(|| model_rotation.map(|_| AttachmentPoseField::ModelRotation));
        Self {
            position: relative_position(rel_position.or(model_position)),
            rotation_wxyz: relative_rotation(rel_rotation.or(model_rotation)),
            selection: AttachmentPoseSelection {
                position,
                rotation,
                model_defaults: model_position.is_some() || model_rotation.is_some(),
            },
        }
    }

    /// Returns the exact selected operands and their coordinate frames.
    #[must_use]
    pub const fn selection(self) -> AttachmentPoseSelection {
        self.selection
    }
}

/// Original operands and coordinate frames of a CDF attachment pose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AttachmentPoseSelection {
    position: Option<AttachmentPoseField>,
    rotation: Option<AttachmentPoseField>,
    model_defaults: bool,
}

impl AttachmentPoseSelection {
    /// Returns whether this authored attribute supplied a selected value.
    #[must_use]
    pub fn uses(self, field: AttachmentPoseField) -> bool {
        self.position == Some(field) || self.rotation == Some(field)
    }

    /// Returns whether this attribute's presence sets the frame of a missing component.
    #[must_use]
    pub const fn selects_default(self, field: AttachmentPoseField) -> bool {
        self.model_defaults
            && match field {
                AttachmentPoseField::ModelPosition => self.rotation.is_none(),
                AttachmentPoseField::ModelRotation => self.position.is_none(),
                AttachmentPoseField::RelativePosition | AttachmentPoseField::RelativeRotation => {
                    false
                }
            }
    }

    /// Returns whether the translation is expressed in the bone frame.
    #[must_use]
    pub const fn position_is_relative(self) -> bool {
        match self.position {
            Some(AttachmentPoseField::ModelPosition) => false,
            Some(_) => true,
            None => !self.model_defaults,
        }
    }

    /// Returns whether the orientation is expressed in the bone frame.
    #[must_use]
    pub const fn rotation_is_relative(self) -> bool {
        match self.rotation {
            Some(AttachmentPoseField::ModelRotation) => false,
            Some(_) => true,
            None => !self.model_defaults,
        }
    }

    /// Returns whether the projection uses the bone's bind pose.
    #[must_use]
    pub const fn uses_bone(self) -> bool {
        self.position_is_relative() || self.rotation_is_relative()
    }

    /// Projects the selected values into the owning model's space.
    #[must_use]
    pub fn in_model_space(
        self,
        position: [f32; 3],
        rotation_wxyz: [f32; 4],
        model_from_bone: &Transform,
    ) -> Option<Transform> {
        let mut authored = attachment_offset(position, rotation_wxyz)?;
        if self.uses_bone() {
            let converted = model_from_bone.then(&authored);
            for (row, [bx, by, bz, bt]) in authored.rows.iter_mut().zip(converted.rows) {
                let [x, y, z, translation] = *row;
                let [x, y, z] = if self.rotation_is_relative() {
                    [bx, by, bz]
                } else {
                    [x, y, z]
                };
                let translation = if self.position_is_relative() {
                    bt
                } else {
                    translation
                };
                *row = [x, y, z, translation];
            }
        }
        authored.finite()
    }
}

#[cfg(test)]
#[path = "character_attachment_pose_tests.rs"]
mod tests;
