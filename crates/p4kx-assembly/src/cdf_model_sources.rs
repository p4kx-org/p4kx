// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_cryxml::SourceAttributePosition;
use p4kx_extraction::SourceLocation;

use crate::cdf_attribute_position::CdfCoordinateDomain;
use crate::loadout_xml::{Node, Scope};

#[derive(Debug, PartialEq)]
enum Selection {
    Attachment {
        binding: SourceAttributePosition,
        skin_type: Option<SourceAttributePosition>,
    },
    File(SourceAttributePosition),
}

#[derive(Debug, PartialEq)]
pub(crate) struct CdfModelSources {
    selection: Selection,
    material: Option<SourceAttributePosition>,
}

impl CdfModelSources {
    pub(crate) fn attachment(
        node: Node<'_, '_>,
        scope: &Scope<'_, '_>,
        preferred_skin: bool,
    ) -> Option<Self> {
        let position = |name| scope.attribute_ascii_position(node, name);
        Some(Self {
            selection: Selection::Attachment {
                binding: position("Binding")?,
                skin_type: preferred_skin.then(|| position("Type")).flatten(),
            },
            material: position("Material"),
        })
    }

    pub(crate) fn file(node: Node<'_, '_>, scope: &Scope<'_, '_>) -> Option<Self> {
        Some(Self {
            selection: Selection::File(scope.attribute_ascii_position(node, "File")?),
            material: None,
        })
    }

    pub(crate) fn model(&self) -> impl Iterator<Item = SourceLocation> {
        let domain = CdfCoordinateDomain::Model;
        match &self.selection {
            Selection::Attachment { binding, skin_type } => [
                domain.location(binding, "Binding"),
                skin_type
                    .as_ref()
                    .and_then(|position| domain.location(position, "Type")),
            ],
            Selection::File(file) => [domain.location(file, "File"), None],
        }
        .into_iter()
        .flatten()
    }

    pub(crate) fn material(&self) -> Option<SourceLocation> {
        CdfCoordinateDomain::Model.location(self.material.as_ref()?, "Material")
    }
}
