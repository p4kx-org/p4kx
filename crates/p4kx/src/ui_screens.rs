// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_assembly::{ArchiveFiles, LoadoutResolver};
use p4kx_extraction::DataForgeEntry;
use p4kx_screens::{EntityScreens, Screen, ScreenComposer};
use serde::Serialize;

use crate::cli::UiScreensArgs;
use crate::{exit, json};

#[path = "ui_screens_origins.rs"]
mod origins;

use self::origins::Evidence;

#[derive(Serialize)]
struct ScreensJson<'a> {
    archive: String,
    entry: &'a str,
    #[serde(flatten)]
    listing: &'a EntityScreens,
    #[serde(flatten)]
    evidence: Evidence,
}

/// Runs `p4kx ui screens`.
pub(crate) fn run(args: &UiScreensArgs, as_json: bool) -> Result<ExitCode> {
    let archive = crate::key::open(&args.archive)?;
    let entry = DataForgeEntry::read(&archive)?;
    let forge = entry.open()?;
    let files = ArchiveFiles::new(&archive);
    let mut resolver = LoadoutResolver::new(&forge).with_files(&files);
    let record = resolver.entities().find(&args.entity)?;
    let tree = resolver.resolve(&record)?;
    let mut composer = ScreenComposer::new(&forge);
    if as_json {
        let observed = composer.screens_of_observed(&tree);
        json::print(&ScreensJson {
            archive: json::path(archive.path()),
            entry: entry.path(),
            listing: observed.listing(),
            evidence: Evidence::of(&entry, &observed)?,
        })?;
    } else {
        match print(&composer.screens_of(&tree)) {
            Err(error) if error.kind() != io::ErrorKind::BrokenPipe => return Err(error.into()),
            _ => {}
        }
    }
    Ok(ExitCode::from(exit::SUCCESS))
}

fn print(listing: &EntityScreens) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(out, "Entity:  {}", listing.entity)?;
    if let Some(brand) = &listing.brand {
        writeln!(out, "Brand:   {brand}")?;
    }
    writeln!(out, "Screens: {}", listing.screens.len())?;
    for screen in &listing.screens {
        writeln!(out)?;
        print_screen(&mut out, screen)?;
    }
    if !listing.movies.is_empty() {
        writeln!(out)?;
        writeln!(out, "UI movies: {}", listing.movies.len())?;
        for movie in &listing.movies {
            writeln!(
                out,
                "{}  {}  {}",
                port_name(&movie.port),
                movie.entity,
                movie.movie
            )?;
        }
    }
    out.flush()
}

const fn port_name(port: &str) -> &str {
    if port.is_empty() { "-" } else { port }
}

fn print_screen(out: &mut impl Write, screen: &Screen) -> io::Result<()> {
    writeln!(
        out,
        "{}  {}  {} x {} px",
        port_name(&screen.port),
        screen.entity,
        screen.width,
        screen.height
    )?;
    writeln!(out, "  canvas:    {}", screen.canvas)?;
    writeln!(out, "  field:     {}", screen.field)?;
    let assets = &screen.assets;
    writeln!(
        out,
        "  uses:      {} canvases, {} textures, {} vector graphics, {} movies, {} localisation keys",
        assets.canvases.len(),
        assets.textures.len(),
        assets.vectors.len(),
        assets.movies.len(),
        assets.localisation_keys.len()
    )?;
    for (label, list) in [
        ("texture:", &assets.textures),
        ("vector:", &assets.vectors),
        ("movie:", &assets.movies),
    ] {
        for path in list {
            writeln!(out, "  {label:<10} {path}")?;
        }
    }
    let elements: Vec<String> = screen
        .elements
        .iter()
        .map(|element| {
            let drawn = if element.rendered { "" } else { " (not drawn)" };
            format!("{} {}{drawn}", element.element, element.count)
        })
        .collect();
    writeln!(out, "  elements:  {}", elements.join(", "))
}
