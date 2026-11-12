// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, FieldOrigin, GlobalIni, InputScope, ScreenTexts,
    SourceEvidenceError, SourceInput, SourceInventory, SourceLocation, SourceScope, SourceUse,
    SourceUseRole, SourceUses,
};
use p4kx_gfx::CanvasSize;

use crate::archive_screen_images::ImageReads;
use crate::still::ObservedStill;
use crate::still_inputs::StillInputs;

use SourceUseRole::{Conversion, Selector};

pub(super) struct ScreenSource<'a, 'text> {
    pub(super) dataforge: &'a DataForgeEntry,
    pub(super) font: Option<u32>,
    pub(super) texts: &'a ScreenTexts,
    pub(super) ini: Option<&'a GlobalIni<'text>>,
}

/// Source evidence for a screen artifact and its independently derived dimensions.
#[derive(Debug)]
#[non_exhaustive]
pub struct ScreenRenderOrigin {
    /// Original uses of the committed opaque PNG or SVG payload.
    pub artifact: FieldOrigin,
    /// The sources of the output width.
    pub width: FieldOrigin,
    /// The sources of the output height.
    pub height: FieldOrigin,
    /// A value supplied directly by the tool or a caller option.
    pub tool_authored: FieldOrigin,
    /// A value whose contributing inputs cannot be established.
    pub unknown: FieldOrigin,
    /// The physical source and use inventory for the artifact and its dimensions.
    pub provenance: SourceInventory,
}

impl ScreenRenderOrigin {
    pub(super) fn register(
        source: &ScreenSource<'_, '_>,
        drawn: &ObservedStill,
        images: &ImageReads<'_>,
        size: CanvasSize,
    ) -> Result<Self, SourceEvidenceError> {
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
        let tool_authored = registry.origin(&[], true);
        let width = match size {
            CanvasSize::Stage => registry.origin(&dimensions[..1], true),
            CanvasSize::Width(_) | CanvasSize::Fit { .. } => tool_authored.clone(),
        };
        let height = match size {
            CanvasSize::Stage => registry.origin(&dimensions[1..], true),
            CanvasSize::Width(_) => registry.origin(&dimensions, true),
            CanvasSize::Fit { .. } => tool_authored.clone(),
        };
        Ok(Self {
            artifact: registry.origin(&actual, complete),
            width,
            height,
            tool_authored,
            unknown: registry.origin(&[], false),
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
    ) -> Result<bool, SourceEvidenceError> {
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

fn dimensions(
    scope: &SourceScope,
    record_index: u32,
) -> Result<[SourceUse; 2], SourceEvidenceError> {
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
