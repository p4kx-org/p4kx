// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

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
            "1.0.0",
            "Exact decoded DDS source Entries remain in the common RawReference. Header metadata retains authored words and separates reader-derived topology from codec support.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "header-metadata",
                "Selected DDS and alpha headers, original legacy and DX10 words, normalized topology, codec capability and expected split-part count.",
            ),
            FieldGroup::new_with_unsupported_projection(
                "sibling-graph",
                "Selected split payloads retain exact raw bytes; physical sibling graph projection is unavailable.",
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
        let alpha = match texture_role(context.entry.path()) {
            Some(TextureMemberRole::Header) => false,
            Some(TextureMemberRole::Member(_, TextureMember::AlphaHeader)) => true,
            _ => return Ok(Projection::Skipped),
        };
        header::project(context, writer, alpha)?;
        Ok(Projection::Written)
    }
}
