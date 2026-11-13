// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

mod family;
mod family_expected;
mod family_origin;
mod family_payload;
mod header;
mod origin;
pub(crate) mod schema;

use p4kx_extraction::{TextureMember, TextureMemberRole, texture_role};

use crate::{
    Capability, FieldGroup, MediaError, MediaKind, MediaWriter, Projection, Provider, SourceContext,
};

pub(crate) struct TextureProvider;

impl Provider for TextureProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Texture,
            "p4kx.texture.dds",
            "1.1.0",
            "Exact decoded DDS source Entries remain in the common RawReference. Header metadata retains authored words and separates reader-derived topology from codec support.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "header-metadata",
                "Selected DDS and alpha headers, original legacy and DX10 words, normalized topology, codec capability and expected split-part count.",
            ),
            FieldGroup::new(
                "sibling-graph",
                "Physical duplicate-aware DDS family members, selection and declared sizes, and the observed disposition of each expected split part.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "texture-conversion",
                "Metadata export does not decode surfaces or write PNG or OpenEXR artifacts.",
            ),
        ];
        capability
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        let path = context.entry.path();
        let header = match texture_role(path) {
            Some(TextureMemberRole::Header) => header::HeaderRole {
                family: path,
                alpha: false,
            },
            Some(TextureMemberRole::Member(family, TextureMember::AlphaHeader)) => {
                header::HeaderRole {
                    family,
                    alpha: true,
                }
            }
            _ => return Ok(Projection::Skipped),
        };
        header::project(context, writer, header)?;
        Ok(Projection::Written)
    }
}
