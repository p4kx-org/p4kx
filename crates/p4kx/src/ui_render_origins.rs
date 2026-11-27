// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use p4kx_extraction::{
    FieldOrigin, FieldOrigins, FieldPath, PublishedUiRender, RenderSummaryOrigins, SourceInventory,
};
use serde::Serialize;

use crate::json;
use crate::ui_render::RenderJson;

pub(crate) struct RenderEvidence {
    receipt: PathBuf,
    artifact: FieldOrigin,
    summary: RenderSummaryOrigins,
    provenance: SourceInventory,
}

impl RenderEvidence {
    pub(crate) fn of<T>(published: PublishedUiRender<T>) -> (T, Self) {
        (
            published.render,
            Self {
                receipt: published.origin_receipt,
                artifact: published.artifact_origin,
                summary: published.summary_origins,
                provenance: published.provenance,
            },
        )
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RenderWithOrigins<'a> {
    #[serde(flatten)]
    render: RenderJson<'a>,
    origin_receipt: String,
    artifact_origin: FieldOrigin,
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

pub(crate) fn print(render: RenderJson<'_>, evidence: RenderEvidence) -> Result<()> {
    let name = evidence
        .receipt
        .file_name()
        .context("missing render receipt name")?;
    let field_origins = fields(&render, &evidence.summary)?;
    json::print(&RenderWithOrigins {
        render,
        origin_receipt: json::path(Path::new(name)),
        artifact_origin: evidence.artifact,
        field_origins,
        provenance: evidence.provenance,
    })?;
    Ok(())
}

fn fields(render: &RenderJson<'_>, evidence: &RenderSummaryOrigins) -> Result<FieldOrigins> {
    let mut origins = FieldOrigins::new();
    for (path, source) in [
        ("/movie", &evidence.movie),
        ("/target", &evidence.tool_authored),
        ("/format", &evidence.tool_authored),
        ("/width", &evidence.width),
        ("/height", &evidence.height),
        ("/skipped", &evidence.unknown),
        ("/failed", &evidence.unknown),
        ("/imports/movies", &evidence.unknown),
        ("/imports/unresolved", &evidence.unknown),
    ] {
        origins.insert(FieldPath::parse(path)?, source.clone())?;
    }
    diagnostics(&mut origins, render, evidence)?;
    imports(&mut origins, render, evidence)?;
    Ok(origins)
}

fn diagnostics(
    origins: &mut FieldOrigins,
    render: &RenderJson<'_>,
    evidence: &RenderSummaryOrigins,
) -> Result<()> {
    for (index, skipped) in render.skipped.iter().enumerate() {
        let row = format!("/skipped/{index}");
        insert_fields(
            origins,
            &row,
            ["id", "depth", "reason", "drawn", "message"],
            &evidence.unknown,
        )?;
        if skipped.sprite.is_some() {
            insert_fields(origins, &row, ["sprite"], &evidence.unknown)?;
        }
    }
    for (index, failed) in render.failed.iter().enumerate() {
        let row = format!("/failed/{index}");
        insert_fields(origins, &row, ["kind"], &evidence.tool_authored)?;
        insert_fields(origins, &row, ["error"], &evidence.unknown)?;
        if failed.id.is_some() {
            insert_fields(origins, &row, ["id"], &evidence.unknown)?;
        }
        let tag = evidence
            .tag_failures
            .get(index)
            .unwrap_or(&evidence.unknown);
        if failed.code.is_some() {
            insert_fields(origins, &row, ["code"], tag)?;
        }
        if failed.offset.is_some() {
            insert_fields(origins, &row, ["offset"], tag)?;
        }
    }
    Ok(())
}

fn imports(
    origins: &mut FieldOrigins,
    render: &RenderJson<'_>,
    evidence: &RenderSummaryOrigins,
) -> Result<()> {
    for index in 0..render.imports.movies.len() {
        let name = evidence
            .import_movies
            .get(index)
            .unwrap_or(&evidence.unknown);
        origins.insert(
            FieldPath::parse(&format!("/imports/movies/{index}"))?,
            name.clone(),
        )?;
    }
    for index in 0..render.imports.unresolved.len() {
        let row = format!("/imports/unresolved/{index}");
        let original = evidence.unresolved_imports.get(index);
        let url = original.map_or(&evidence.unknown, |input| &input.url);
        let name = original.map_or(&evidence.unknown, |input| &input.name);
        insert_fields(origins, &row, ["url"], url)?;
        insert_fields(origins, &row, ["name"], name)?;
        insert_fields(origins, &row, ["reason", "message"], &evidence.unknown)?;
    }
    Ok(())
}

fn insert_fields<const COUNT: usize>(
    origins: &mut FieldOrigins,
    path: &str,
    names: [&str; COUNT],
    origin: &FieldOrigin,
) -> Result<()> {
    let parent = FieldPath::parse(path)?;
    for name in names {
        let mut field = parent.clone();
        field.push_field(name)?;
        origins.insert(field, origin.clone())?;
    }
    Ok(())
}
