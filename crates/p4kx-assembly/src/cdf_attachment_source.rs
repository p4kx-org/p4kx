// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;

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
    /// The material override.
    Material,
}

impl CdfAttachmentField {
    const FIELDS: [Self; 6] = [
        Self::Type,
        Self::Binding,
        Self::BoneName,
        Self::RelativePosition,
        Self::RelativeRotation,
        Self::Material,
    ];

    const fn name(self) -> &'static str {
        match self {
            Self::Type => "Type",
            Self::Binding => "Binding",
            Self::BoneName => "BoneName",
            Self::RelativePosition => "RelPosition",
            Self::RelativeRotation => "RelRotation",
            Self::Material => "Material",
        }
    }

    /// Returns whether this attribute supplies an attachment offset.
    #[must_use]
    pub const fn is_offset(self) -> bool {
        matches!(self, Self::RelativePosition | Self::RelativeRotation)
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
        let (namespace, indices) = match &self.position {
            SourceAttributePosition::Text(range) => (
                "cdfAttachmentTextBytes",
                vec![
                    u64::try_from(range.start).ok()?,
                    u64::try_from(range.end).ok()?,
                ],
            ),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => (
                "cdfAttachmentCryXmlAttribute",
                vec![u64::from(*node_index), u64::from(*attribute_index)],
            ),
            _ => return None,
        };
        Some(SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: self.field.name().into(),
        })
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
