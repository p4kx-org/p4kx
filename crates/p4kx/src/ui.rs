// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{Language, LocalizationEntry, UiMovie};

use crate::cli::{UiAction, UiArgs};
use crate::cli_ui::UiSource;
use crate::ui_extract::extract;
use crate::ui_hologram;
use crate::ui_json::{AssetsJson, describe};
use crate::ui_render::render;
use crate::{exit, ui_asset_origins, ui_screen_render, ui_screens};

const NONE: &str = "-";

/// Runs `p4kx ui assets`, `p4kx ui extract`, `p4kx ui render` or `p4kx ui screens`.
pub(crate) fn run(args: &UiArgs, as_json: bool) -> Result<ExitCode> {
    match &args.action {
        UiAction::Screens(screens) => ui_screens::run(screens, as_json),
        UiAction::Hologram(hologram) => ui_hologram::run(hologram, as_json),
        UiAction::Assets(source) => {
            with_movie(source, None, as_json, |movie| assets(movie, as_json))
        }
        UiAction::Extract { source, out } => {
            with_movie(source, None, true, |movie| extract(movie, out, as_json))
        }
        UiAction::Render {
            source,
            screen: Some(screen),
            brand,
            variables,
            out,
            size,
            background,
            svg,
            lang,
        } => {
            let options = ui_screen_render::Options {
                screen,
                brand: brand.as_deref(),
                variables,
                lang: *lang,
                out,
                size: *size,
                background: *background,
                svg: *svg,
            };
            ui_screen_render::run(&source.path, &options, as_json)
        }
        UiAction::Render {
            source,
            out,
            size,
            background,
            svg,
            lang,
            ..
        } => with_movie(source, *lang, false, |movie| {
            render(movie, out, *size, *background, *svg, as_json)
        }),
    }
}

fn with_movie(
    source: &UiSource,
    lang: Option<Language>,
    observe_file: bool,
    run: impl FnOnce(&UiMovie<'_>) -> Result<ExitCode>,
) -> Result<ExitCode> {
    match &source.entry {
        Some(entry) => {
            let archive = crate::key::open(&source.path)?;
            let movie = UiMovie::from_archive(&archive, entry)?;
            let movie = match lang {
                Some(lang) => movie.with_texts(LocalizationEntry::read(&archive, lang)?),
                None => movie,
            };
            run(&movie)
        }
        None if observe_file => run(&UiMovie::from_file_observed(&source.path)?),
        None => run(&UiMovie::from_file(&source.path)?),
    }
}

fn assets(movie: &UiMovie<'_>, as_json: bool) -> Result<ExitCode> {
    let complete = if as_json {
        ui_asset_origins::print(movie)?
    } else {
        let assets = movie.movie().assets();
        print_assets(&describe(movie, &assets))?;
        assets.failures.is_empty()
    };
    Ok(ExitCode::from(if complete {
        exit::SUCCESS
    } else {
        exit::PARTIAL_FAILURE
    }))
}

fn print_assets(assets: &AssetsJson<'_>) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(out, "Movie:        {}", assets.movie)?;
    writeln!(
        out,
        "Format:       {} version {}, {}, {} bytes uncompressed",
        assets.signature,
        assets.version,
        match assets.compression {
            "none" => "not compressed",
            "zlib" => "zlib-compressed",
            "lzma" => "LZMA-compressed",
            other => other,
        },
        assets.length
    )?;
    writeln!(
        out,
        "Stage:        {} x {} px, {} fps, {} frames",
        pixels(assets.stage.width()),
        pixels(assets.stage.height()),
        assets.frame_rate,
        assets.frame_count
    )?;
    if let Some(exporter) = &assets.exporter {
        writeln!(
            out,
            "Exporter:     GFx {:#06X}, from {}",
            exporter.version, exporter.swf_name
        )?;
    }
    let tags: u32 = assets.tags.iter().map(|tag| tag.count).sum();
    writeln!(
        out,
        "Tags:         {tags}, {} unknown, {} unreadable",
        assets.unknown_tags.len(),
        assets.failures.len()
    )?;
    writeln!(out)?;
    writeln!(out, "{:>6}  {:<30}  {:>7}", "CODE", "TAG", "COUNT")?;
    for tag in &assets.tags {
        let name = tag.name.unwrap_or("unknown");
        writeln!(out, "{:>6}  {name:<30}  {:>7}", tag.code, tag.count)?;
    }
    print_definitions(&mut out, assets)?;
    for tag in &assets.unknown_tags {
        writeln!(
            out,
            "unknown tag {} at offset {}, {} bytes",
            tag.code, tag.offset, tag.length
        )?;
    }
    for failure in &assets.failures {
        writeln!(
            out,
            "unreadable tag {} at offset {}: {}",
            failure.code, failure.offset, failure.error
        )?;
    }
    out.flush()
}

fn print_definitions(out: &mut impl Write, assets: &AssetsJson<'_>) -> io::Result<()> {
    section(
        out,
        "Shapes",
        assets.shapes.len() + assets.morph_shapes.len(),
    )?;
    for (kind, shapes) in [
        ("shape", &assets.shapes),
        ("morph shape", &assets.morph_shapes),
    ] {
        for shape in shapes {
            writeln!(
                out,
                "{:>6}  {kind:<12} v{}  {} x {} px, {} edges  {}",
                shape.id,
                shape.version,
                pixels(shape.bounds.width()),
                pixels(shape.bounds.height()),
                shape.edges,
                shape.export_name.unwrap_or(NONE)
            )?;
        }
    }
    print_bitmaps(out, assets)?;
    section(out, "Fonts", assets.fonts.len())?;
    for font in &assets.fonts {
        writeln!(
            out,
            "{:>6}  {:<20} {:>5} glyphs  {}",
            font.id, font.format, font.glyphs, font.name
        )?;
    }
    section(out, "Sprites", assets.sprites.len())?;
    for sprite in &assets.sprites {
        writeln!(
            out,
            "{:>6}  {:>5} frames, {} placed  {}",
            sprite.id,
            sprite.frames,
            sprite.placed.len(),
            sprite.export_name.unwrap_or(NONE)
        )?;
    }
    Ok(())
}

fn print_bitmaps(out: &mut impl Write, assets: &AssetsJson<'_>) -> io::Result<()> {
    section(out, "Bitmaps", assets.bitmaps.len())?;
    for bitmap in &assets.bitmaps {
        let size = match (bitmap.width, bitmap.height) {
            (Some(width), Some(height)) => format!("{width} x {height}"),
            _ => NONE.to_owned(),
        };
        let detail = match (bitmap.file, bitmap.image_id) {
            (Some(file), _) => file.to_owned(),
            (None, Some(image)) => format!("of bitmap {image}"),
            (None, None) => bitmap.format.clone().unwrap_or_default(),
        };
        writeln!(
            out,
            "{:>6}  {:<12} {size:>11}  {detail}  {}",
            bitmap.id,
            bitmap.kind,
            bitmap.export_name.unwrap_or(NONE)
        )?;
    }
    Ok(())
}

fn pixels(twips: i64) -> String {
    let whole = twips / 20;
    match twips % 20 {
        0 => whole.to_string(),
        rest => format!(
            "{}{whole}.{:02}",
            if twips < 0 && whole == 0 { "-" } else { "" },
            rest.abs() * 5
        ),
    }
}

fn section(out: &mut impl Write, title: &str, count: usize) -> io::Result<()> {
    writeln!(out)?;
    writeln!(out, "{title}: {count}")
}

/// Returns an error and its causes joined by `: `.
pub(crate) fn chain(error: &(dyn std::error::Error + 'static)) -> String {
    let mut text = String::new();
    let mut current = Some(error);
    while let Some(error) = current {
        if !text.is_empty() {
            text.push_str(": ");
        }
        text.push_str(&error.to_string());
        current = error.source();
    }
    text
}
