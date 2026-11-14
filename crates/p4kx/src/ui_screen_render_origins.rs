// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{FieldOrigin, FieldOrigins, FieldPath, SourceInventory};
use p4kx_screens::{ScreenRenderOrigin, Still};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ScreenEvidence<'a> {
    artifact_origin: &'a FieldOrigin,
    field_origins: FieldOrigins,
    provenance: &'a SourceInventory,
}

impl<'a> ScreenEvidence<'a> {
    pub(super) fn of(still: &Still, source: &'a ScreenRenderOrigin) -> Result<Self> {
        Ok(Self {
            artifact_origin: &source.artifact,
            field_origins: summary_fields(still, source)?,
            provenance: &source.provenance,
        })
    }
}

fn summary_fields(still: &Still, source: &ScreenRenderOrigin) -> Result<FieldOrigins> {
    let tool = &source.tool_authored;
    let unknown = &source.unknown;
    let mut fields = FieldOrigins::new();
    for (path, origin) in [
        ("/screen", tool),
        ("/target", tool),
        ("/format", tool),
        ("/width", &source.width),
        ("/height", &source.height),
        ("/widgets", unknown),
        ("/shapes", unknown),
        ("/canvases", unknown),
        ("/texts_drawn", unknown),
        ("/texts_left_out", unknown),
        ("/images_drawn", unknown),
        ("/images_left_out", unknown),
        ("/truncated", unknown),
        ("/missing_canvases", unknown),
        ("/unsupported", unknown),
        ("/skipped", unknown),
    ] {
        fields.insert(FieldPath::parse(path)?, origin.clone())?;
    }
    diagnostics(&mut fields, still, tool, unknown)?;
    Ok(fields)
}

fn diagnostics(
    fields: &mut FieldOrigins,
    still: &Still,
    tool: &FieldOrigin,
    unknown: &FieldOrigin,
) -> Result<()> {
    let report = still.report();
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
    for (index, _) in still.skipped().iter().enumerate() {
        for name in ["reason", "message"] {
            fields.insert(
                FieldPath::parse(&format!("/skipped/{index}/{name}"))?,
                unknown.clone(),
            )?;
        }
    }
    Ok(())
}
