// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::MaterialSourceLimits;

use crate::material_projection;
use crate::{
    Capability, FieldGroup, MediaError, MediaKind, MediaWriter, Projection, Provider, SourceContext,
};

/// Streams standalone material libraries from centrally retained source bytes.
#[derive(Debug, Clone, Copy, Default)]
pub struct MaterialProvider {
    limits: MaterialSourceLimits,
}

impl MaterialProvider {
    /// Sets bounded material parsing independently of the common generation limits.
    #[must_use]
    pub const fn new_with_limits(limits: MaterialSourceLimits) -> Self {
        Self { limits }
    }
}

impl Provider for MaterialProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Material,
            "p4kx.material.library",
            "1.1.0",
            "Exact original decoded MTL bytes remain solely in the common RawReference; bounded UTF8/CryXml authored root syntax and existing interpreted surfaces are separate projections, without geometry conversion; selected shader-reference inputs are read explicitly with their own receipts.",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "authored-library-root",
                "ordered root attributes, namespace declarations, QName, original submaterial positions, unknown blocks and actual truncation boundaries; document prolog remains in raw bytes",
            ),
            FieldGroup::new(
                "plain-mixed-content",
                "original UTF8 text lexemes and ordered text/CDATA-derived content, comments, processing instructions and original element ordinals",
            ),
            FieldGroup::new(
                "binary-node-content",
                "binary node strings are separate from children because no mixed-content interleaving is encoded; actual repaired string bytes are diagnosed",
            ),
            FieldGroup::new(
                "material-source-fields",
                "ordered PublicParams/layers/unknown attributes/elements/texture slots and TexMod children, preserving raw tokens alongside interpreted finite/default values",
            ),
            FieldGroup::new_with_unsupported_projection(
                "binary-namespace-expansion",
                "binary QName and namespace-declaration strings are retained without inventing expanded namespace URIs",
            ),
            FieldGroup::new(
                "physical-dependency-identities",
                "complete owning-source inventories classify original layer/texture file references with duplicate physical candidates and selection; readability and raw projection remain target-owned",
            ),
            FieldGroup::new(
                "shader-definition-links",
                "original shader and selected flag tokens resolve through the bounded complete root inventory and local Property declarations; references retain actual input receipts without runtime shader evaluation",
            ),
            FieldGroup::new_with_unsupported_projection(
                "runtime-placeholder-and-palette-bindings",
                "engine texture placeholders and DataForge palette tokens are preserved without an inferred file candidate or palette GUID",
            ),
            FieldGroup::new_with_unsupported_projection(
                "runtime-material-semantics",
                "retaining shader/mask/flag tokens does not execute shaders, establish in-game appearance or validate arbitrary Blender shader graphs",
            ),
        ];
        capability
    }

    fn source(
        &self,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Projection, MediaError> {
        writer.checkpoint()?;
        let Some((_, extension)) = context.entry.path().rsplit_once('.') else {
            return Ok(Projection::Skipped);
        };
        if !extension.eq_ignore_ascii_case("mtl") {
            return Ok(Projection::Skipped);
        }
        material_projection::project(context, self.limits, writer)
    }
}
