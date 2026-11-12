// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;

use crate::loadout_xml::AttributeNamespaces;

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
        let namespaces = match self {
            Self::Attachment => AttributeNamespaces {
                text_bytes: "cdfAttachmentTextBytes",
                cryxml_attribute: "cdfAttachmentCryXmlAttribute",
            },
            Self::Model => AttributeNamespaces {
                text_bytes: "cdfModelTextBytes",
                cryxml_attribute: "cdfModelCryXmlAttribute",
            },
        };
        namespaces.location(position, field)
    }
}
