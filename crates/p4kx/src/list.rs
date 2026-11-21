// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use anyhow::Result;
use p4kx_extraction::{Archive, Entry, TextureGroup, TextureGroups};
use serde::Serialize;
use std::io::{self, BufWriter, Write};

use crate::cli::{ListArgs, SearchArgs};
use crate::entry_listing_json::EntryRows;
use crate::filter_pair;
use crate::json;

#[derive(Serialize)]
struct EntriesJson<'a> {
    archive: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    filter: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    exclude: &'a [String],
    entries: EntryRows<'a>,
}

/// Prints every Entry of the Archive; the parts of a split texture are folded into the row of
/// its `.dds` header unless `--parts` is given.
pub(crate) fn run(args: &ListArgs, as_json: bool) -> Result<()> {
    let archive = crate::key::open(&args.archive)?;
    crate::issues::warn(&archive)?;
    if args.parts {
        let entries: Vec<&Entry> = archive.entries().iter().collect();
        return print(&archive, None, &entries, as_json);
    }
    let groups = TextureGroups::build(&archive);
    if as_json {
        json::print(&GroupedJson {
            archive: json::path(archive.path()),
            entries: EntryRows::grouped(&archive, &groups),
        })?;
        return Ok(());
    }
    let rows: Vec<Row<'_>> = archive
        .entries()
        .iter()
        .enumerate()
        .filter(|&(position, _)| !groups.is_part(position))
        .map(|(position, entry)| Row {
            entry,
            group: groups
                .group_of(position)
                .map(|group| (group, group_paths(&archive, group))),
        })
        .collect();
    print_rows(&rows)?;
    Ok(())
}

struct Row<'a> {
    entry: &'a Entry,
    group: Option<(&'a TextureGroup, Vec<&'a str>)>,
}

fn group_paths<'a>(archive: &'a Archive, group: &TextureGroup) -> Vec<&'a str> {
    group
        .parts()
        .filter_map(|position| archive.entries().get(position).map(Entry::path))
        .collect()
}

#[derive(Serialize)]
struct GroupedJson<'a> {
    archive: String,
    entries: EntryRows<'a>,
}

fn print_rows(rows: &[Row<'_>]) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    write_row(
        &mut out,
        ["METHOD", "ENCRYPTED", "COMPRESSED", "SIZE", "PATH"],
    )?;
    for Row { entry, group } in rows {
        let encrypted = if entry.is_encrypted() { "yes" } else { "no" };
        let (compressed, size) = group.as_ref().map_or_else(
            || (entry.compressed_size(), entry.uncompressed_size()),
            |(group, _)| (group.compressed_size(), group.uncompressed_size()),
        );
        let path = group.as_ref().map_or_else(
            || entry.path().to_owned(),
            |(_, parts)| {
                let count = parts.len();
                let noun = if count == 1 { "part" } else { "parts" };
                format!("{} (+{count} {noun})", entry.path())
            },
        );
        write_row(
            &mut out,
            [
                &entry.compression().to_string(),
                encrypted,
                &compressed.to_string(),
                &size.to_string(),
                &path,
            ],
        )?;
    }
    out.flush()
}

/// Prints the Entries of the Archive that match the Filter and no exclusion Filter.
pub(crate) fn search(args: &SearchArgs, as_json: bool) -> Result<()> {
    let archive = crate::key::open(&args.archive)?;
    let selected = filter_pair::search(&archive, &args.filter, &args.exclude)?;
    print(
        &archive,
        Some((&args.filter, &args.exclude)),
        &selected.entries,
        as_json,
    )
}

fn print(
    archive: &Archive,
    filters: Option<(&str, &[String])>,
    entries: &[&Entry],
    as_json: bool,
) -> Result<()> {
    if as_json {
        json::print(&EntriesJson {
            archive: json::path(archive.path()),
            filter: filters.map(|(include, _)| include),
            exclude: filters.map_or(&[][..], |(_, exclude)| exclude),
            entries: EntryRows::physical(archive, entries),
        })?;
    } else {
        print_table(entries)?;
    }
    Ok(())
}

/// Prints a header and one row per Entry: method, encrypted, compressed size, size, path.
pub(crate) fn print_table(entries: &[&Entry]) -> io::Result<()> {
    let mut out = BufWriter::new(io::stdout().lock());
    write_row(
        &mut out,
        ["METHOD", "ENCRYPTED", "COMPRESSED", "SIZE", "PATH"],
    )?;
    for entry in entries {
        let encrypted = if entry.is_encrypted() { "yes" } else { "no" };
        write_row(
            &mut out,
            [
                &entry.compression().to_string(),
                encrypted,
                &entry.compressed_size().to_string(),
                &entry.uncompressed_size().to_string(),
                entry.path(),
            ],
        )?;
    }
    out.flush()
}

fn write_row(
    out: &mut impl Write,
    [method, encrypted, compressed, size, path]: [&str; 5],
) -> io::Result<()> {
    writeln!(
        out,
        "{method:<10} {encrypted:<9} {compressed:>14} {size:>14}  {path}"
    )
}
