// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{FieldPath, InputScope, SourceUseRole};

use crate::material_reference::{MaterialReference, MaterialReferenceRole};
use crate::origin_capture::OriginSite;
use crate::origin_contributions::OriginContributions;
use crate::origin_input::MediaOriginInput;
use crate::origin_prepared::PreparedRecordOrigins;
use crate::origin_reference::{
    self, ReferenceIdentifier, ReferenceOriginInputs, ReferenceSubresource,
};
use crate::reference_resolve::ReferenceObservations;
use crate::{MediaError, MediaWriter, Reference, SourceContext};

pub(crate) struct MaterialResolution {
    pub(crate) reference: Reference,
    pub(crate) origins: MaterialReferenceProof,
}

pub(crate) struct MaterialReferenceProof {
    pub(crate) evidence: ReferenceObservations<MediaOriginInput>,
    pub(crate) inventory: OriginContributions,
}

pub(crate) fn write(
    prefix: &FieldPath,
    reference: &MaterialReference,
    resolution: &MaterialReferenceProof,
    context: &SourceContext<'_>,
    prepared: &mut PreparedRecordOrigins,
    writer: &MediaWriter<'_>,
) -> Result<(), MediaError> {
    let declaration = DeclarationOrigins::capture(reference, context, writer)?;

    let semantic = OriginContributions::independent(&writer.origins.owner);
    let inputs = ReferenceOriginInputs {
        original: &declaration.original,
        selector: &declaration.selector,
        inventory: &resolution.inventory,
        semantic: &semantic,
        candidates: &resolution.evidence,
        read_selected: false,
        identifier: if matches!(reference.role, MaterialReferenceRole::EnginePlaceholder) {
            ReferenceIdentifier::Text
        } else {
            ReferenceIdentifier::Path
        },
        subresource: ReferenceSubresource::None,
    };
    prepared.insert(prefix, inputs.resolution(writer)?, writer)?;
    declaration.scalars(reference, prepared, prefix, writer)?;
    inputs.write(
        prepared,
        &origin_reference::child(prefix, "reference")?,
        &reference.reference,
        writer,
    )
}

struct DeclarationOrigins {
    input: MediaOriginInput,
    indices: Vec<u64>,
    namespace: &'static str,
    original: OriginContributions,
    selector: OriginContributions,
}

impl DeclarationOrigins {
    fn capture(
        reference: &MaterialReference,
        context: &SourceContext<'_>,
        writer: &MediaWriter<'_>,
    ) -> Result<Self, MediaError> {
        let input = context.origin_input(writer)?;
        let indices = indices(reference)?;
        let (namespace, field) = match reference.role {
            MaterialReferenceRole::DefaultPalette => ("material.xml", "DefaultPalette"),
            MaterialReferenceRole::LayerLibrary => ("material-layer-declaration", "Path"),
            MaterialReferenceRole::Texture | MaterialReferenceRole::EnginePlaceholder => {
                ("material-texture-declaration", "File")
            }
        };
        let observe = |role| -> Result<OriginContributions, MediaError> {
            let mut capture = writer.capture_origins();
            capture.add(
                &input,
                InputScope::Payload,
                OriginSite::new(namespace, &indices, field),
                role,
                writer,
            )?;
            Ok(capture.finish())
        };
        let original = observe(SourceUseRole::Payload)?;
        let selector = observe(SourceUseRole::Selector)?;
        Ok(Self {
            input,
            indices,
            namespace,
            original,
            selector,
        })
    }

    fn scalars(
        &self,
        reference: &MaterialReference,
        prepared: &mut PreparedRecordOrigins,
        prefix: &FieldPath,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for field in ["role", "material", "slot"] {
            prepared.insert_subtree(
                &origin_reference::child(prefix, field)?,
                self.selector.clone(),
                writer,
            )?;
        }
        let submaterial = origin_reference::child(prefix, "submaterial")?;
        if matches!(reference.role, MaterialReferenceRole::LayerLibrary) {
            let mut capture = writer.capture_origins();
            let role = if reference.submaterial.is_some() {
                SourceUseRole::Payload
            } else {
                SourceUseRole::Selector
            };
            capture.add(
                &self.input,
                InputScope::Payload,
                OriginSite::new(self.namespace, &self.indices, "Submtl"),
                role,
                writer,
            )?;
            prepared.insert_subtree(&submaterial, capture.finish(), writer)?;
        } else {
            origin_reference::independent(prepared, &submaterial, writer)?;
        }
        Ok(())
    }
}
fn indices(reference: &MaterialReference) -> Result<Vec<u64>, MediaError> {
    reference
        .material
        .into_iter()
        .chain(reference.slot)
        .map(crate::shader_values::count)
        .collect()
}
