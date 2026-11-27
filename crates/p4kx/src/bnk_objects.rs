// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::hirc_select::{self, LayoutCounts, ObjectPage};
use p4kx_extraction::hirc_view::ObjectView;
use p4kx_extraction::{Bank, FieldOrigins, SourceInventory};
use serde::Serialize;

use crate::bnk::{self, open};
use crate::bnk_objects_text::write_object;
use crate::cli::BnkObjectsArgs;
use crate::{exit, json};

#[derive(Serialize)]
struct ObjectsJson<'a> {
    soundbank: &'a str,
    version: u32,
    bank_id: u32,
    total: usize,
    layouts: LayoutCounts,
    matched: usize,
    offset: usize,
    limit: usize,
    next_offset: Option<usize>,
    objects: Vec<ObjectView<'a>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

/// Runs `p4kx bnk objects`.
pub(crate) fn run(args: &BnkObjectsArgs, as_json: bool) -> Result<ExitCode> {
    let bank = if as_json {
        bnk::open_observed(&args.source)?
    } else {
        open(&args.source)?
    };
    let objects = bank.soundbank().objects();
    let offset = usize::try_from(args.offset)?;
    let limit = usize::try_from(args.limit)?;
    let counts = LayoutCounts::of(objects);
    if as_json {
        let selection = hirc_select::select_with_origins(&bank, args.kind, args.id, offset, limit)?;
        let page = selection.page;
        json::print(&ObjectsJson {
            soundbank: bank.name(),
            version: bank.soundbank().header().version,
            bank_id: bank.soundbank().header().id,
            total: objects.len(),
            layouts: counts,
            matched: page.matched,
            offset,
            limit,
            next_offset: page.next_offset(),
            objects: page.shown,
            field_origins: selection.field_origins,
            provenance: selection.provenance,
        })?;
        return Ok(ExitCode::from(exit::SUCCESS));
    }
    let page = hirc_select::select_with_sources(&bank, args.kind, args.id, offset, limit);
    match print_text(&bank, &counts, &page) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(ExitCode::from(exit::SUCCESS)),
    }
}

fn print_text(bank: &Bank, counts: &LayoutCounts, page: &ObjectPage<'_>) -> io::Result<()> {
    let soundbank = bank.soundbank();
    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(out, "Soundbank:   {}", bank.name())?;
    writeln!(out, "Version:     {}", soundbank.header().version)?;
    writeln!(out, "Bank ID:     {}", soundbank.header().id)?;
    if !page.shown.is_empty() {
        writeln!(out)?;
    }
    for object in &page.shown {
        write_object(&mut out, object)?;
    }
    writeln!(out, "{}", summary(soundbank.objects().len(), counts, page))?;
    out.flush()
}

fn summary(total: usize, counts: &LayoutCounts, page: &ObjectPage<'_>) -> String {
    let shown = if page.shown.is_empty() {
        format!("showing none of {}", page.matched)
    } else {
        format!(
            "showing {}-{} of {}",
            page.offset.saturating_add(1),
            page.offset.saturating_add(page.shown.len()),
            page.matched
        )
    };
    let next = page.next_offset().map_or_else(String::new, |offset| {
        format!("; next page: --offset {offset}")
    });
    format!(
        "{total} objects ({} decoded, {} undecoded, {} unsupported, {} malformed); {} match: \
         {shown}{next}",
        counts.decoded, counts.undecoded, counts.unsupported, counts.malformed, page.matched
    )
}
