// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::HashMap;

use anyhow::Result;
use p4kx_extraction::gfx::CanvasSize;
use p4kx_extraction::texture_preview::ObservedMipInput;
use p4kx_extraction::{
    ArchiveImages, ConversionError, DataForgeEntry, DataForgeOrigin, FieldOrigin, FieldOrigins,
    FieldPath, GlobalIni, InputScope, ObservedEntry, ScreenTexts, SourceInput, SourceInventory,
    SourceLocation, SourceScope, SourceUse, SourceUseRole, SourceUses,
};
use p4kx_screens::{MAX_SVG_BYTES, ObservedStill, StillInputs};
use serde::Serialize;

use SourceUseRole::{Conversion, Selector};

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
    ) -> Result<p4kx_extraction::RgbaImage, ConversionError> {
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

    fn register(
        &self,
        inputs: &StillInputs,
        registry: &SourceUses,
        actual: &mut Vec<SourceUse>,
    ) -> Result<bool> {
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

pub(super) struct ScreenSource<'a, 'text> {
    pub(super) dataforge: &'a DataForgeEntry,
    pub(super) font: Option<u32>,
    pub(super) texts: &'a ScreenTexts,
    pub(super) ini: Option<&'a GlobalIni<'text>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScreenEvidence {
    pub(super) artifact_origin: FieldOrigin,
    field_origins: FieldOrigins,
    pub(super) provenance: SourceInventory,
}

impl ScreenEvidence {
    pub(super) fn register(
        source: &ScreenSource<'_, '_>,
        drawn: &ObservedStill,
        images: &ImageReads<'_>,
        size: CanvasSize,
    ) -> Result<Self> {
        let registry = SourceUses::new();
        let input = match source.dataforge.origin() {
            DataForgeOrigin::Archive(entry) => SourceInput::Archive(entry),
            _ => SourceInput::Unknown,
        };
        let scope = registry.observe(input, InputScope::Payload)?;
        let dataforge = scope.at(
            SourceLocation::Subresource {
                namespace: "dataForgeBytes".into(),
                indices: vec![0, source.dataforge.byte_len()],
                field: "screenInput".into(),
            },
            Conversion,
        )?;
        let dimensions = dimensions(&scope, drawn.inputs.canvas_record)?;
        let mut actual = vec![dataforge];
        actual.extend_from_slice(&dimensions);
        let mut complete = drawn.still.is_complete();
        complete &= images.register(&drawn.inputs, &registry, &mut actual)?;
        complete &= source.register_text(&drawn.inputs, &scope, &registry, &mut actual)?;
        let fields = summary_fields(drawn, size, &registry, &dimensions)?;
        Ok(Self {
            artifact_origin: registry.origin(&actual, complete),
            field_origins: fields,
            provenance: registry.freeze()?,
        })
    }
}

impl ScreenSource<'_, '_> {
    fn register_text(
        &self,
        inputs: &StillInputs,
        forge: &SourceScope,
        registry: &SourceUses,
        actual: &mut Vec<SourceUse>,
    ) -> Result<bool> {
        if !inputs.text.is_empty()
            && let Some(record_index) = self.font
        {
            actual.push(forge.at(
                SourceLocation::DataForge {
                    record_index,
                    field: "fontFile".into(),
                },
                Selector,
            )?);
        }
        let mut complete = true;
        for input in &inputs.text {
            complete &= self.texts.append_font_uses(input.font, registry, actual)?;
            if let Some(key) = &input.localization_key {
                if let Some(ini) = self.ini {
                    complete &= self
                        .texts
                        .append_localization_uses(ini, key, registry, actual)?;
                } else {
                    complete = false;
                }
            }
        }
        Ok(complete)
    }
}

fn dimensions(scope: &SourceScope, record_index: u32) -> Result<[SourceUse; 2]> {
    let dimension = |field: &str| {
        scope.at(
            SourceLocation::DataForge {
                record_index,
                field: field.into(),
            },
            Conversion,
        )
    };
    Ok([dimension("size.x")?, dimension("size.y")?])
}

fn summary_fields(
    drawn: &ObservedStill,
    size: CanvasSize,
    registry: &SourceUses,
    dimensions: &[SourceUse; 2],
) -> Result<FieldOrigins> {
    let tool = registry.origin(&[], true);
    let unknown = registry.origin(&[], false);
    let width = match size {
        CanvasSize::Stage => registry.origin(&dimensions[..1], true),
        CanvasSize::Width(_) | CanvasSize::Fit { .. } => tool.clone(),
    };
    let height = match size {
        CanvasSize::Stage => registry.origin(&dimensions[1..], true),
        CanvasSize::Width(_) => registry.origin(dimensions, true),
        CanvasSize::Fit { .. } => tool.clone(),
    };
    let mut fields = FieldOrigins::new();
    for (path, origin) in [
        ("/screen", &tool),
        ("/target", &tool),
        ("/format", &tool),
        ("/width", &width),
        ("/height", &height),
        ("/widgets", &unknown),
        ("/shapes", &unknown),
        ("/canvases", &unknown),
        ("/texts_drawn", &unknown),
        ("/texts_left_out", &unknown),
        ("/images_drawn", &unknown),
        ("/images_left_out", &unknown),
        ("/truncated", &unknown),
        ("/missing_canvases", &unknown),
        ("/unsupported", &unknown),
        ("/skipped", &unknown),
    ] {
        fields.insert(FieldPath::parse(path)?, origin.clone())?;
    }
    diagnostics(&mut fields, drawn, &tool, &unknown)?;
    Ok(fields)
}

fn diagnostics(
    fields: &mut FieldOrigins,
    drawn: &ObservedStill,
    tool: &FieldOrigin,
    unknown: &FieldOrigin,
) -> Result<()> {
    let report = drawn.still.report();
    if !report.missing_textures.is_empty() {
        fields.insert(FieldPath::parse("/missing_textures")?, unknown.clone())?;
    }
    if !report.variables.is_empty() {
        fields.insert(FieldPath::parse("/variables")?, unknown.clone())?;
    }
    for (path, count) in [
        ("/missing_canvases", report.missing_canvases.len()),
        ("/missing_textures", report.missing_textures.len()),
    ] {
        for index in 0..count {
            fields.insert(
                FieldPath::parse(&format!("{path}/{index}"))?,
                unknown.clone(),
            )?;
        }
    }
    for (index, entry) in report.unsupported.iter().enumerate() {
        for name in ["element", "reason", "count", "examples"] {
            fields.insert(
                FieldPath::parse(&format!("/unsupported/{index}/{name}"))?,
                unknown.clone(),
            )?;
        }
        for example in 0..entry.examples.len() {
            fields.insert(
                FieldPath::parse(&format!("/unsupported/{index}/examples/{example}"))?,
                unknown.clone(),
            )?;
        }
    }
    for (index, _) in report.variables.iter().enumerate() {
        for (name, origin) in [("name", tool), ("value", tool), ("used", unknown)] {
            fields.insert(
                FieldPath::parse(&format!("/variables/{index}/{name}"))?,
                origin.clone(),
            )?;
        }
    }
    for (index, _) in drawn.still.skipped().iter().enumerate() {
        for name in ["reason", "message"] {
            fields.insert(
                FieldPath::parse(&format!("/skipped/{index}/{name}"))?,
                unknown.clone(),
            )?;
        }
    }
    Ok(())
}
