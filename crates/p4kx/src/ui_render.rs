// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;
use std::process::ExitCode;

use crate::ui_render_origins::{self, RenderEvidence};
use anyhow::Result;
use p4kx_extraction::gfx::{Background, CanvasSize, RasterOptions, Skipped, TagFailure};
use p4kx_extraction::{ImportReport, UiBitmapFailure, UiMovie};
use serde::Serialize;

use crate::ui::chain;
use crate::{exit, json};

#[derive(Serialize)]
pub(super) struct RenderJson<'a> {
    movie: &'a str,
    target: String,
    format: &'static str,
    width: u32,
    height: u32,
    pub(super) skipped: Vec<SkippedJson>,
    pub(super) failed: Vec<FailedJson>,
    pub(super) imports: ImportsJson,
}

#[derive(Serialize)]
pub(super) struct ImportsJson {
    pub(super) movies: Vec<String>,
    pub(super) unresolved: Vec<UnresolvedJson>,
}

#[derive(Serialize)]
pub(super) struct UnresolvedJson {
    url: String,
    name: String,
    reason: &'static str,
    message: String,
}

#[derive(Serialize)]
pub(super) struct SkippedJson {
    id: u16,
    depth: u16,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) sprite: Option<u16>,
    reason: &'static str,
    drawn: bool,
    message: String,
}

#[derive(Serialize)]
pub(super) struct FailedJson {
    kind: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) id: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) offset: Option<usize>,
    error: String,
}

/// What a render left out, whichever format it wrote.
struct Parts {
    width: u32,
    height: u32,
    skipped: Vec<Skipped>,
    bitmap_failures: Vec<UiBitmapFailure>,
    tag_failures: Vec<TagFailure>,
    imports: ImportReport,
}

impl Parts {
    const fn is_complete(&self) -> bool {
        self.skipped.is_empty() && self.bitmap_failures.is_empty() && self.tag_failures.is_empty()
    }
}

/// Runs `p4kx ui render` on an opened movie.
pub(crate) fn render(
    movie: &UiMovie<'_>,
    out: &Path,
    size: Option<CanvasSize>,
    background: Background,
    svg: bool,
    as_json: bool,
) -> Result<ExitCode> {
    let options = RasterOptions {
        size: size.unwrap_or(CanvasSize::Stage),
        background,
    };
    let (parts, evidence) = render_parts(movie, out, &options, svg)?;
    let skipped: Vec<SkippedJson> = parts.skipped.iter().map(skipped_json).collect();
    let failed = failed(&parts);
    if as_json {
        ui_render_origins::print(
            RenderJson {
                movie: movie.name(),
                target: json::path(out),
                format: if svg { "svg" } else { "png" },
                width: parts.width,
                height: parts.height,
                skipped,
                failed,
                imports: ImportsJson {
                    movies: parts.imports.movies.clone(),
                    unresolved: parts
                        .imports
                        .unresolved
                        .iter()
                        .map(|import| UnresolvedJson {
                            url: import.url.clone(),
                            name: import.name.clone(),
                            reason: import.issue.code(),
                            message: import.issue.to_string(),
                        })
                        .collect(),
                },
            },
            evidence,
        )?;
    } else {
        print_report(movie, out, &parts, &skipped, &failed)?;
    }
    Ok(ExitCode::from(if parts.is_complete() {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn render_parts(
    movie: &UiMovie<'_>,
    out: &Path,
    options: &RasterOptions,
    svg: bool,
) -> Result<(Parts, RenderEvidence)> {
    Ok(if svg {
        let (written, evidence) =
            RenderEvidence::of(movie.render_svg_to_file_with_origin(options, out)?);
        (
            Parts {
                width: written.width,
                height: written.height,
                skipped: written.skipped,
                bitmap_failures: written.bitmap_failures,
                tag_failures: written.tag_failures,
                imports: written.imports,
            },
            evidence,
        )
    } else {
        let (rendered, evidence) =
            RenderEvidence::of(movie.render_to_file_with_origin(options, out)?);
        (
            Parts {
                width: rendered.image.width(),
                height: rendered.image.height(),
                skipped: rendered.skipped,
                bitmap_failures: rendered.bitmap_failures,
                tag_failures: rendered.tag_failures,
                imports: rendered.imports,
            },
            evidence,
        )
    })
}

fn print_report(
    movie: &UiMovie<'_>,
    out: &Path,
    parts: &Parts,
    skipped: &[SkippedJson],
    failed: &[FailedJson],
) -> io::Result<()> {
    let mut stderr = io::stderr().lock();
    for entry in skipped {
        let place = entry.sprite.map_or_else(
            || format!("depth {}", entry.depth),
            |sprite| format!("depth {} of sprite {sprite}", entry.depth),
        );
        let action = if entry.drawn {
            "drawn without an effect"
        } else {
            "skipped"
        };
        writeln!(
            stderr,
            "{action}: definition {} at {place}: {}",
            entry.id, entry.message
        )?;
    }
    for import in &parts.imports.unresolved {
        writeln!(
            stderr,
            "import not found: {} from {}: {}",
            import.name, import.url, import.issue
        )?;
    }
    for failure in failed {
        match (failure.code, failure.offset) {
            (Some(code), Some(offset)) => writeln!(
                stderr,
                "error: unreadable tag {code} at offset {offset}: {}",
                failure.error
            )?,
            _ => writeln!(stderr, "error: {}", failure.error)?,
        }
    }
    drop(stderr);
    writeln!(
        io::stdout().lock(),
        "rendered {} into {}, {} x {} px; {} elements left out, {} bitmaps and {} tags unreadable",
        movie.name(),
        out.display(),
        parts.width,
        parts.height,
        skipped.len(),
        parts.bitmap_failures.len(),
        parts.tag_failures.len()
    )
}

fn skipped_json(skipped: &Skipped) -> SkippedJson {
    SkippedJson {
        id: skipped.id,
        depth: skipped.depth,
        sprite: skipped.sprite,
        reason: skipped.reason.code(),
        drawn: skipped.reason.is_effect(),
        message: skipped.reason.to_string(),
    }
}

fn failed(parts: &Parts) -> Vec<FailedJson> {
    let tags = parts.tag_failures.iter().map(|failure| FailedJson {
        kind: "tag",
        id: None,
        code: Some(failure.code),
        offset: Some(failure.offset),
        error: chain(&failure.error),
    });
    let bitmaps = parts.bitmap_failures.iter().map(|failure| FailedJson {
        kind: "bitmap",
        id: Some(failure.id),
        code: None,
        offset: None,
        error: chain(&failure.error),
    });
    tags.chain(bitmaps).collect()
}
