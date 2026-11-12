// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;

#[derive(Clone, Copy)]
pub(crate) enum CdfCoordinateDomain {
    Attachment,
    Model,
}

impl CdfCoordinateDomain {
    pub(crate) fn location(
        self,
        position: &SourceAttributePosition,
        field: &str,
    ) -> Option<SourceLocation> {
        let (namespace, indices) = match position {
            SourceAttributePosition::Text(range) => (
                match self {
                    Self::Attachment => "cdfAttachmentTextBytes",
                    Self::Model => "cdfModelTextBytes",
                },
                vec![
                    u64::try_from(range.start).ok()?,
                    u64::try_from(range.end).ok()?,
                ],
            ),
            SourceAttributePosition::CryXml {
                node_index,
                attribute_index,
            } => (
                match self {
                    Self::Attachment => "cdfAttachmentCryXmlAttribute",
                    Self::Model => "cdfModelCryXmlAttribute",
                },
                vec![u64::from(*node_index), u64::from(*attribute_index)],
            ),
            _ => return None,
        };
        Some(SourceLocation::Subresource {
            namespace: namespace.into(),
            indices,
            field: field.into(),
        })
    }
}
