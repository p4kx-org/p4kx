// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use crate::shader_payload::{ShaderCache, ShaderCacheDecode, ShaderCacheKind, ShaderPayload};
use crate::{
    Capability, FieldGroup, MediaError, MediaIssue, MediaKind, MediaWriter, Projection, Provider,
    Severity, SourceContext, Status, Subresource,
};
use crate::{shader_projection, shader_source};

/// Stateless projection of source extension declarations and explicit compiled-cache inventory.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShaderProvider {
    limits: p4kx_shader::Limits,
}

impl ShaderProvider {
    /// Sets bounded per-source lexical work in addition to the common writer budgets.
    #[must_use]
    pub const fn new_with_limits(limits: p4kx_shader::Limits) -> Self {
        Self { limits }
    }
}

impl Provider for ShaderProvider {
    fn capability(&self) -> Capability {
        let mut capability = Capability::new(
            MediaKind::Shader,
            "p4kx.shader.extension",
            "1.0.0",
            "centrally retained exact decoded Entry bytes; original Entry-relative byte spans; no shader execution or compiled-cache semantic decoding",
        );
        capability.field_groups = vec![
            FieldGroup::new(
                "ordered-source-syntax",
                "bounded UTF-8 .ext declarations, duplicates, unknown scopes, raw values and exact byte spans",
            ),
            FieldGroup::new(
                "source-field-views",
                "Version/common flags and direct Property fields retain occurrence order, authored form and optional value presence",
            ),
            FieldGroup::new(
                "explicit-mask-literals",
                "only explicit 0x/0X unsigned 64-bit words or unambiguous zero; exact decimal strings and diagnosed noninterpretation states",
            ),
            FieldGroup::new_with_unsupported_projection(
                "general-mask-evaluation",
                "unprefixed nonzero, symbolic/quoted/compound expressions and words beyond unsigned64 remain raw; no engine-wide interpretation is promised",
            ),
            FieldGroup::new(
                "compiled-cache-inventory",
                "selected .cfib/.cfxb/.shcb family, retained source and separate cache counting units only",
            ),
            FieldGroup::new_with_unsupported_projection(
                "compiled-cache-semantics",
                "compiled payloads are inventoried with semantic_decode=unsupported; no definition or Property is inferred",
            ),
            FieldGroup::new_with_unsupported_projection(
                "material-definition-links",
                "material links require a separate bounded complete-inventory lookup",
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
        if extension.eq_ignore_ascii_case("ext") {
            return shader_source::project(context, self.limits, writer);
        }
        let family = if extension.eq_ignore_ascii_case("cfib") {
            ShaderCacheKind::Cfib
        } else if extension.eq_ignore_ascii_case("cfxb") {
            ShaderCacheKind::Cfxb
        } else if extension.eq_ignore_ascii_case("shcb") {
            ShaderCacheKind::Shcb
        } else {
            return Ok(Projection::Skipped);
        };
        let data = ShaderPayload::CompiledCache(ShaderCache {
            family,
            semantic_decode: ShaderCacheDecode::Unsupported,
            compiled_caches: 1,
            definitions: 0,
            properties: 0,
        });
        writer.record(
            &Subresource::new(shader_projection::CACHE_NAMESPACE, Vec::new()),
            Status::Complete,
            &data,
            &[MediaIssue::new(
                "shader.cache.unsupported-semantics",
                "cache.semantic_decode",
                Severity::Info,
                "Raw cache inventory is complete; compiled shader semantics are not decoded.",
            )],
        )?;
        Ok(Projection::Written)
    }
}
