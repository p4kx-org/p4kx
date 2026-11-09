// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::material_shader_reference::{MaterialShaderReference, MaterialShaderRole};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{
    self, ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource,
};
use crate::shader_origins::{ShaderCandidateInput, ShaderReferenceOrigins};
use crate::{InputReceipt, MediaError, MediaWriter, SourceContext, origin_error};

pub(crate) fn write(
    position: usize,
    reference: &MaterialShaderReference,
    shader_attribute: Option<usize>,
    origins: &ShaderReferenceOrigins,
    context: &SourceContext<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let usage = Usage::capture(reference, shader_attribute, context, writer)?;
    let mut prefix = FieldPath::parse("/data/payload/shader_references")
        .map_err(|cause| origin_error::address(&cause))?;
    prefix
        .push_index(position)
        .map_err(|cause| origin_error::address(&cause))?;
    let inputs = usage.reference_inputs(reference, origins);
    prepared.insert(&prefix, inputs.resolution(writer)?, writer)?;
    usage.scalars(prepared, &prefix, reference, writer)?;
    inputs.write(
        prepared,
        &origin_reference::child(&prefix, "reference")?,
        &reference.reference,
        writer,
    )?;
    receipt(
        prepared,
        &origin_reference::child(&prefix, "input")?,
        reference,
        origins,
        &usage,
        writer,
    )
}

struct Usage {
    input: MediaOriginInput,
    shader_indices: Vec<u64>,
    original: OriginContributions,
    selector: OriginContributions,
    shader_selector: OriginContributions,
}

impl Usage {
    fn capture(
        reference: &MaterialShaderReference,
        shader_attribute: Option<usize>,
        context: &SourceContext<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let input = context.origin_input(writer)?;
        let mut indices = vec![
            crate::shader_values::count(reference.material)?,
            crate::shader_values::count(reference.attribute_index)?,
        ];
        if let Some(token) = reference.token_index {
            indices.push(token);
        }
        let field = if matches!(reference.role, MaterialShaderRole::Shader) {
            "Shader"
        } else {
            "StringGenMask"
        };
        let original = source_use(&input, &indices, field, SourceUseRole::Payload, writer)?;
        let mut shader_indices = vec![crate::shader_values::count(reference.material)?];
        if let Some(attribute) = shader_attribute {
            shader_indices.push(crate::shader_values::count(attribute)?);
        }
        let shader_selector = source_use(
            &input,
            &shader_indices,
            "Shader",
            SourceUseRole::Selector,
            writer,
        )?;
        let mut selector = writer.capture_origins();
        selector.include(
            &source_use(&input, &indices, field, SourceUseRole::Selector, writer)?,
            writer,
        )?;
        selector.include(&shader_selector, writer)?;
        Ok(Self {
            input,
            shader_indices,
            original,
            selector: selector.finish(),
            shader_selector,
        })
    }

    const fn reference_inputs<'a>(
        &'a self,
        reference: &MaterialShaderReference,
        origins: &'a ShaderReferenceOrigins,
    ) -> ReferenceOriginInputs<'a, ShaderCandidateInput> {
        ReferenceOriginInputs {
            original: &self.original,
            selector: &self.selector,
            inventory: &origins.inventory,
            semantic: &origins.semantic,
            candidates: &origins.candidates,
            read_selected: origins.read_selected,
            identifier: ReferenceIdentifier::Text,
            subresource: if matches!(reference.role, MaterialShaderRole::Shader) {
                ReferenceSubresource::Definition
            } else {
                ReferenceSubresource::Property
            },
        }
    }

    fn scalars(
        &self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        reference: &MaterialShaderReference,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for field in ["material", "attribute_index", "role"] {
            prepared.insert_subtree(
                &origin_reference::child(prefix, field)?,
                self.selector.clone(),
                writer,
            )?;
        }
        let token_index = origin_reference::child(prefix, "token_index")?;
        if reference.token_index.is_some() {
            prepared.insert_subtree(&token_index, self.selector.clone(), writer)?;
        } else {
            origin_reference::independent(prepared, &token_index, writer)?;
        }
        let mut shader = writer.capture_origins();
        shader.include(&self.shader_selector, writer)?;
        if reference.shader.is_some() {
            shader.include(
                &source_use(
                    &self.input,
                    &self.shader_indices,
                    "Shader",
                    SourceUseRole::Payload,
                    writer,
                )?,
                writer,
            )?;
        }
        prepared.insert_subtree(
            &origin_reference::child(prefix, "shader")?,
            shader.finish(),
            writer,
        )
    }
}

fn source_use(
    input: &MediaOriginInput,
    indices: &[u64],
    field: &str,
    role: SourceUseRole,
    writer: &MediaWriter<'_>,
) -> Result<OriginContributions, MediaError> {
    let mut capture = writer.capture_origins();
    capture.add(
        input,
        InputScope::Payload,
        OriginSite::new("material-shader-reference", indices, field),
        role,
        writer,
    )?;
    Ok(capture.finish())
}

fn receipt(
    prepared: &mut PreparedRecordOrigins,
    prefix: &FieldPath,
    reference: &MaterialShaderReference,
    origins: &ShaderReferenceOrigins,
    usage: &Usage,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let mut availability = writer.capture_origins();
    availability.include(&usage.selector, writer)?;
    if origins.candidates.inventory_used || origins.read_selected {
        availability.include(&origins.inventory, writer)?;
    }
    let Some(receipt) = &reference.input else {
        availability.include(&origins.semantic, writer)?;
        return prepared.insert_subtree(prefix, availability.finish(), writer);
    };
    let context = ReceiptOrigins { receipt, usage };
    let digest = context.digest(writer)?;
    availability.include(&digest, writer)?;
    prepared.insert(prefix, availability.finish(), writer)?;
    context.fields(prepared, prefix, digest, writer)
}

struct ReceiptOrigins<'a> {
    receipt: &'a InputReceipt,
    usage: &'a Usage,
}

impl ReceiptOrigins<'_> {
    fn original(&self) -> Result<&MediaOriginInput, MediaError> {
        self.receipt.origin.as_ref().ok_or_else(|| {
            MediaError::Contract("material shader receipt lost its original input".into())
        })
    }

    fn digest(&self, writer: &MediaWriter<'_>) -> Result<OriginContributions, MediaError> {
        let mut digest = writer.capture_origins();
        digest.include(&self.usage.shader_selector, writer)?;
        digest.add(
            self.original()?,
            InputScope::Payload,
            OriginSite::new("media-physical-input", &[], "complete-payload"),
            SourceUseRole::Conversion,
            writer,
        )?;
        Ok(digest.finish())
    }

    fn fields(
        &self,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        digest: OriginContributions,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        crate::origin_prepared_source::source(
            prepared,
            &origin_reference::child(prefix, "source")?,
            &self.receipt.source,
            self.original()?,
            &self.usage.shader_selector,
            writer,
        )?;
        let digest_path = origin_reference::child(prefix, "digest")?;
        prepared.insert_subtree(&digest_path, digest, writer)?;
        for field in ["algorithm", "scope"] {
            origin_reference::independent(
                prepared,
                &origin_reference::child(&digest_path, field)?,
                writer,
            )?;
        }
        origin_reference::independent(
            prepared,
            &origin_reference::child(prefix, "omission")?,
            writer,
        )?;
        let use_at = origin_reference::child(prefix, "use_at")?;
        prepared.insert_subtree(&use_at, self.usage.selector.clone(), writer)?;
        origin_reference::independent(
            prepared,
            &origin_reference::child(&use_at, "namespace")?,
            writer,
        )
    }
}
