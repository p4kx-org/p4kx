// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;

use p4kx_extraction::texture_preview::ObservedMipInput;
use p4kx_extraction::{
    ArchiveImages, ConversionError, InputScope, ObservedEntry, RgbaImage, SourceEvidenceError,
    SourceInput, SourceLocation, SourceUse, SourceUseRole, SourceUses,
};

use crate::still_inputs::StillInputs;
use crate::svg::MAX_SVG_BYTES;

use SourceUseRole::Conversion;

#[derive(Default)]
pub(super) struct ImageReads<'archive> {
    textures: RefCell<HashMap<String, ObservedMipInput<'archive>>>,
    vectors: RefCell<HashMap<String, VectorInput<'archive>>>,
}

struct VectorInput<'archive> {
    source: Option<ObservedEntry<'archive>>,
    length: u64,
}

impl<'archive> ImageReads<'archive> {
    pub(super) fn texture(
        &self,
        images: &'archive ArchiveImages<'_>,
        path: &str,
        side: u32,
        pixels: u64,
    ) -> Result<RgbaImage, ConversionError> {
        let (image, input) = images.texture_observed(path, side, pixels)?.into_parts();
        self.textures
            .borrow_mut()
            .insert(path.to_ascii_lowercase(), input);
        Ok(image)
    }

    pub(super) fn vector(
        &self,
        images: &'archive ArchiveImages<'_>,
        path: &str,
    ) -> Result<Vec<u8>, ConversionError> {
        let observed = images.vector_observed(path, MAX_SVG_BYTES as u64)?;
        self.vectors.borrow_mut().insert(
            path.to_ascii_lowercase(),
            VectorInput {
                source: observed.origin(),
                length: observed.bytes().len() as u64,
            },
        );
        Ok(observed.into_bytes())
    }

    pub(super) fn register(
        &self,
        inputs: &StillInputs,
        registry: &SourceUses,
        actual: &mut Vec<SourceUse>,
    ) -> Result<bool, SourceEvidenceError> {
        let mut complete = true;
        let textures = self.textures.borrow();
        for path in &inputs.textures {
            if let Some(observed) = textures.get(&path.to_ascii_lowercase()) {
                let evidence = observed.evidence(registry)?;
                evidence.append_pixel_uses(actual)?;
                actual.extend(evidence.alpha_lookup_uses()?);
            } else {
                complete = false;
            }
        }
        let vectors = self.vectors.borrow();
        for path in &inputs.vectors {
            let Some(observed) = vectors.get(&path.to_ascii_lowercase()) else {
                complete = false;
                continue;
            };
            complete &= observed.source.is_some();
            let input = observed
                .source
                .map_or(SourceInput::Unknown, SourceInput::ArchiveView);
            let scope = registry.observe(input, InputScope::Payload)?;
            actual.push(scope.at(
                SourceLocation::Subresource {
                    namespace: "svgBytes".into(),
                    indices: vec![0, observed.length],
                    field: "vectorInput".into(),
                },
                Conversion,
            )?);
        }
        Ok(complete)
    }
}
