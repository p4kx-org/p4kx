// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::model::{MaterialLibrary, RawElement};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::material_shader_tokens::{self, Tokens};
use crate::shader_lookup::ShaderBinding;
use crate::{
    InputReceipt, MediaError, MediaWriter, OriginalIdentifier, Reference, ReferenceUnavailable,
    SourceContext, Subresource,
};

/// Original MTL token role, without assigning a rendered material effect.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum MaterialShaderRole {
    /// The material's original shader-name attribute.
    Shader,
    /// One selected `StringGenMask` token in original order.
    Flag,
}

/// A source-preserving MTL link to a physical shader declaration or local Property.
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
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
) -> Result<u64, MediaError> {
    let mut output = ReferenceOutput {
        batch: crate::material_part_batch::Batch::new(
            context,
            crate::material_part_batch::Band::Shader,
        ),
    };
    for index in 0..library.materials.len() {
        writer.checkpoint()?;
        if let Some(node) = material_shader_tokens::node(library, index) {
            project_node(index, node, context, &mut output, writer)?;
        }
    }
    output.batch.finish(writer)
}

struct ReferenceOutput<'a, 'source> {
    batch: crate::material_part_batch::Batch<'a, 'source>,
}

impl ReferenceOutput<'_, '_> {
    fn emit(
        &mut self,
        reference: &MaterialShaderReference,
        shader_attribute: Option<usize>,
        origins: &crate::shader_origins::ShaderReferenceOrigins,
        context: &SourceContext<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let mut prepared = crate::material_parts_origins::prepared(context, writer)?;
        crate::material_shader_origins::write(
            &crate::material_parts_origins::payload()?,
            reference,
            shader_attribute,
            origins,
            context,
            &mut prepared,
            writer,
        )?;
        self.batch.push(reference, prepared, writer)
    }
}

fn project_node(
    index: usize,
    node: &RawElement,
    context: &SourceContext<'_>,
    output: &mut ReferenceOutput<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    let unique = material_shader_tokens::attributes(node, "Shader").count() == 1;
    let mut scope = FlagScope {
        shader: None,
        shader_attribute: None,
        binding: None,
    };
    for (attribute_index, attribute) in material_shader_tokens::attributes(node, "Shader") {
        let resolved = project_shader(index, attribute_index, attribute, context, output, writer)?;
        if unique {
            scope.shader = Some(attribute.value.as_str());
            scope.shader_attribute = Some(attribute_index);
            scope.binding = resolved;
        }
    }
    project_flags(index, node, context, &scope, output, writer)
}

fn project_shader<'lookup>(
    index: usize,
    attribute_index: usize,
    attribute: &p4kx_extraction::model::AuthoredAttribute,
    context: &SourceContext<'lookup>,
    output: &mut ReferenceOutput<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<ShaderBinding<'lookup>>, MediaError> {
    let use_at = use_at(index, attribute_index, None)?;
    let resolved = context
        .directory
        .shaders
        .map(|lookup| lookup.shader(&attribute.value, &use_at, writer))
        .transpose()?;
    if let Some(value) = &resolved {
        output.emit(
            &MaterialShaderReference {
                material: index,
                attribute_index,
                token_index: None,
                role: MaterialShaderRole::Shader,
                shader: Some(attribute.value.clone()),
                reference: value.reference.clone(),
                input: value.input.clone(),
            },
            Some(attribute_index),
            &value.origins,
            context,
            writer,
        )?;
    } else {
        let value = unavailable(
            "p4kx.shader.name",
            &attribute.value,
            ReferenceUnavailable::TargetNotRead,
            writer,
        )?;
        output.emit(
            &MaterialShaderReference {
                material: index,
                attribute_index,
                token_index: None,
                role: MaterialShaderRole::Shader,
                shader: Some(attribute.value.clone()),
                reference: value.reference,
                input: value.input,
            },
            Some(attribute_index),
            &value.origins,
            context,
            writer,
        )?;
    }
    Ok(resolved)
}

struct FlagScope<'source, 'lookup> {
    shader: Option<&'source str>,
    shader_attribute: Option<usize>,
    binding: Option<ShaderBinding<'lookup>>,
}

fn project_flags(
    index: usize,
    node: &RawElement,
    context: &SourceContext<'_>,
    scope: &FlagScope<'_, '_>,
    output: &mut ReferenceOutput<'_, '_>,
    writer: &mut MediaWriter<'_>,
) -> Result<(), MediaError> {
    for (attribute_index, attribute) in material_shader_tokens::attributes(node, "StringGenMask") {
        for (ordinal, token) in Tokens::new(&attribute.value).enumerate() {
            let ordinal = crate::shader_values::count(ordinal)?;
            let location = use_at(index, attribute_index, Some(ordinal))?;
            let resolved = match (context.directory.shaders, scope.binding.as_ref()) {
                (Some(lookup), Some(binding)) => lookup.flag(binding, token, &location, writer)?,
                _ => unavailable(
                    "p4kx.shader.flag",
                    token,
                    ReferenceUnavailable::UnsupportedLayout,
                    writer,
                )?,
            };
            output.emit(
                &MaterialShaderReference {
                    material: index,
                    attribute_index,
                    token_index: Some(ordinal),
                    role: MaterialShaderRole::Flag,
                    shader: scope.shader.map(str::to_owned),
                    reference: resolved.reference,
                    input: resolved.input,
                },
                scope.shader_attribute,
                &resolved.origins,
                context,
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
) -> Result<crate::shader_origins::ShaderFlagBinding, MediaError> {
    crate::shader_origins::unavailable(
        OriginalIdentifier::try_new_with_text(namespace, token)?,
        reason,
        writer,
    )
}
