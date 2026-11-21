// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;
use p4kx_extraction::model::character_definition::AttachmentPoseField;

use crate::cdf_attribute_position::CdfCoordinateDomain;
use crate::loadout_xml::{Node, Scope};

/// An authored CDF attribute that can supply an admitted bone attachment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CdfAttachmentField {
    /// The bone-attachment type.
    Type,
    /// The selected model path.
    Binding,
    /// The requested bone name.
    BoneName,
    /// The relative translation.
    RelativePosition,
    /// The relative rotation.
    RelativeRotation,
    /// The model-space translation.
    ModelPosition,
    /// The model-space rotation.
    ModelRotation,
    /// The material override.
    Material,
}

impl CdfAttachmentField {
    const FIELDS: [Self; 8] = [
        Self::Type,
        Self::Binding,
        Self::BoneName,
        Self::RelativePosition,
        Self::RelativeRotation,
        Self::ModelPosition,
        Self::ModelRotation,
        Self::Material,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Binding => "Binding",
            Self::BoneName => "BoneName",
            Self::Material => "Material",
            Self::RelativePosition => AttachmentPoseField::RelativePosition.name(),
            Self::RelativeRotation => AttachmentPoseField::RelativeRotation.name(),
            Self::ModelPosition => AttachmentPoseField::ModelPosition.name(),
            Self::ModelRotation => AttachmentPoseField::ModelRotation.name(),
        }
    }

    pub(crate) const fn pose_field(self) -> Option<AttachmentPoseField> {
        match self {
            Self::RelativePosition => Some(AttachmentPoseField::RelativePosition),
            Self::RelativeRotation => Some(AttachmentPoseField::RelativeRotation),
            Self::ModelPosition => Some(AttachmentPoseField::ModelPosition),
            Self::ModelRotation => Some(AttachmentPoseField::ModelRotation),
            Self::Type | Self::Binding | Self::BoneName | Self::Material => None,
        }
    }

    /// Returns whether this attribute supplies a position or rotation.
    #[must_use]
    pub const fn is_offset(self) -> bool {
        self.pose_field().is_some()
    }
}

/// One exact original CDF attribute retained from an admitted attachment declaration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CdfAttachmentSite {
    field: CdfAttachmentField,
    position: SourceAttributePosition,
}

impl CdfAttachmentSite {
    /// Returns the field's role in the authored attachment.
    #[must_use]
    pub const fn field(&self) -> CdfAttachmentField {
        self.field
    }

    /// Returns the original text span or CryXml table position.
    #[must_use]
    pub fn location(&self) -> Option<SourceLocation> {
        CdfCoordinateDomain::Attachment.location(&self.position, self.field.name())
    }
}

pub(crate) fn capture(node: Node<'_, '_>, scope: &Scope<'_, '_>) -> Arc<[CdfAttachmentSite]> {
    CdfAttachmentField::FIELDS
        .into_iter()
        .filter_map(|field| {
            Some(CdfAttachmentSite {
                field,
                position: scope.attribute_ascii_position(node, field.name())?,
            })
        })
        .collect()
}
