// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{MaterialLibrary, RawElement};
use schemars::JsonSchema;
use serde::Serialize;

use crate::material_shader_tokens::{self, Tokens};
use crate::shader_lookup::ShaderBinding;
use crate::{
    CandidateInventory, InputReceipt, MediaError, MediaWriter, OriginalIdentifier, Reference,
    ReferenceUnavailable, SourceContext, Subresource,
};

/// Original MTL token role, without assigning a rendered material effect.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialShaderRole {
    /// The material's original shader-name attribute.
    Shader,
    /// One selected `StringGenMask` token in original order.
    Flag,
}

/// A source-preserving MTL link to a physical shader declaration or local Property.
#[derive(Debug, Serialize, JsonSchema)]
#[schemars(extend("allOf" = [
    {"required": ["token_index", "shader", "input"]},
    {"if": {"properties": {"role": {"const": "shader"}}, "required": ["role"]}, "then": {"properties": {"token_index": {"type": "null"}, "shader": {"type": "string"}}}},
    {"if": {"properties": {"role": {"const": "flag"}}, "required": ["role"]}, "then": {"properties": {"token_index": {"type": "integer"}}}}
]))]
#[non_exhaustive]
pub struct MaterialShaderReference {
    /// Original material index in the MTL library.
    pub material: usize,
    /// Original attribute position in the material's authored attribute list.
    pub attribute_index: usize,
    /// Original selected-token position, null for the shader name.
    pub token_index: Option<u64>,
    /// Token role, separate from the target's resolution state.
    pub role: MaterialShaderRole,
    /// The sole original shader name, null when no unique authored scope exists.
    pub shader: Option<String>,
    /// Original token and central resolution classification.
    pub reference: Reference,
    /// Actual verified lookup input; absent when no target bytes were consumed.
    pub input: Option<InputReceipt>,
}

pub(crate) fn project(
    library: &MaterialLibrary,
    context: &SourceContext<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Vec<MaterialShaderReference>, MediaError> {
    let mut references = Vec::new();
    let mut remaining = writer.limits().record_bytes;
    for index in 0..library.materials.len() {
        writer.checkpoint()?;
        if let Some(node) = material_shader_tokens::node(library, index) {
            project_node(
                index,
                node,
                context,
                &mut references,
                &mut remaining,
                writer,
            )?;
        }
    }
    Ok(references)
}

fn project_node(
    index: usize,
    node: &RawElement,
    context: &SourceContext<'_>,
    references: &mut Vec<MaterialShaderReference>,
    remaining: &mut u64,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut shader = None;
    let unique = material_shader_tokens::attributes(node, "Shader").count() == 1;
    let mut binding = None;
    for (attribute_index, attribute) in material_shader_tokens::attributes(node, "Shader") {
        let use_at = use_at(index, attribute_index, None)?;
        let resolved = context
            .directory
            .shaders
            .map(|lookup| lookup.shader(&attribute.value, &use_at, writer))
            .transpose()?;
        let (reference, input) = if let Some(resolved) = &resolved {
            (resolved.reference.clone(), resolved.input.clone())
        } else {
            (
                unavailable(
                    "p4kx.shader.name",
                    &attribute.value,
                    ReferenceUnavailable::TargetNotRead,
                    writer,
                )?,
                None,
            )
        };
        append(
            references,
            remaining,
            MaterialShaderReference {
                material: index,
                attribute_index,
                token_index: None,
                role: MaterialShaderRole::Shader,
                shader: Some(attribute.value.clone()),
                reference,
                input,
            },
            writer,
        )?;
        if unique {
            shader = Some(attribute.value.as_str());
            binding = resolved;
        }
    }
    project_flags(
        index,
        node,
        context,
        &FlagScope {
            shader,
            binding: binding.as_ref(),
        },
        references,
        remaining,
        writer,
    )
}

struct FlagScope<'a, 'lookup> {
    shader: Option<&'a str>,
    binding: Option<&'a ShaderBinding<'lookup>>,
}

fn project_flags(
    index: usize,
    node: &RawElement,
    context: &SourceContext<'_>,
    scope: &FlagScope<'_, '_>,
    references: &mut Vec<MaterialShaderReference>,
    remaining: &mut u64,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (attribute_index, attribute) in material_shader_tokens::attributes(node, "StringGenMask") {
        for (ordinal, token) in Tokens::new(&attribute.value).enumerate() {
            let ordinal = crate::shader_values::count(ordinal)?;
            let location = use_at(index, attribute_index, Some(ordinal))?;
            let (reference, input) = match (context.directory.shaders, scope.binding) {
                (Some(lookup), Some(binding)) => lookup.flag(binding, token, &location, writer)?,
                _ => (
                    unavailable(
                        "p4kx.shader.flag",
                        token,
                        ReferenceUnavailable::UnsupportedLayout,
                        writer,
                    )?,
                    None,
                ),
            };
            append(
                references,
                remaining,
                MaterialShaderReference {
                    material: index,
                    attribute_index,
                    token_index: Some(ordinal),
                    role: MaterialShaderRole::Flag,
                    shader: scope.shader.map(str::to_owned),
                    reference,
                    input,
                },
                writer,
            )?;
        }
    }
    Ok(())
}

fn use_at(
    material: usize,
    attribute: usize,
    token: Option<u64>,
) -> Result<Subresource, MediaError> {
    let mut indices = vec![
        crate::shader_values::count(material)?,
        crate::shader_values::count(attribute)?,
    ];
    if let Some(token) = token {
        indices.push(token);
    }
    Ok(Subresource::new("material-shader-reference", indices))
}

fn unavailable(
    namespace: &str,
    token: &str,
    reason: ReferenceUnavailable,
    writer: &mut MediaWriter<'_>,
) -> Result<Reference, MediaError> {
    writer.resolve_reference(
        OriginalIdentifier::try_new_with_text(namespace, token)?,
        std::iter::empty(),
        CandidateInventory::Unavailable(reason),
    )
}

fn append(
    references: &mut Vec<MaterialShaderReference>,
    remaining: &mut u64,
    reference: MaterialShaderReference,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    use std::io::Write as _;
    let mut measured = crate::artifact::BoundedLine::new(*remaining);
    let serialized = serde_json::to_writer(&mut measured, &reference);
    crate::json_output::finish(serialized, writer.checkpoint)?;
    measured.write_all(b",\n").map_err(|_| MediaError::Limit {
        budget: "record_bytes",
        limit: writer.limits().record_bytes,
    })?;
    *remaining -= crate::shader_values::count(measured.bytes.len())?;
    references
        .try_reserve_exact(1)
        .map_err(|cause| MediaError::Io {
            path: writer.root.clone(),
            source: std::io::Error::new(std::io::ErrorKind::OutOfMemory, cause),
        })?;
    references.push(reference);
    Ok(())
}
