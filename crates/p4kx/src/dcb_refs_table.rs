// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io::{self, Write};

use crate::dcb_refs::{Edges, Info, Link, Listing, Mode};
use crate::query_table::one_line;

const GAP: &str = "  ";

/// Writes the listings of `mode` as aligned tables, each with its heading and counts.
pub(crate) fn listings(
    out: &mut impl Write,
    mode: Mode,
    listings: &[Listing<'_, '_>],
) -> io::Result<()> {
    let incoming = mode == Mode::Incoming;
    for (position, listing) in listings.iter().enumerate() {
        if position > 0 {
            writeln!(out)?;
        }
        let direction = if incoming { "to" } else { "from" };
        writeln!(out, "References {direction} {}", heading(&listing.target))?;
        let rows: Vec<Vec<String>> = listing
            .references
            .iter()
            .map(|link| {
                let record = link.record.as_ref();
                vec![
                    one_line(record.map_or("", |info| info.record_path)),
                    one_line(record.map_or("", |info| info.name)),
                    one_line(&link.location),
                ]
            })
            .collect();
        let other = if incoming { "referring" } else { "referred to" };
        table(
            out,
            &[&format!("{other} record path"), "name", "field"],
            &rows,
        )?;
        let records = distinct(&listing.references);
        writeln!(
            out,
            "{} from {}",
            count(listing.references.len(), "Reference"),
            count(records, "Record")
        )?;
        if !listing.unresolved.is_empty() {
            writeln!(out)?;
            writeln!(out, "Unresolved References")?;
            let rows: Vec<Vec<String>> = listing
                .unresolved
                .iter()
                .map(|link| vec![guid(link), one_line(&link.location)])
                .collect();
            table(out, &["GUID", "field"], &rows)?;
            writeln!(
                out,
                "{}",
                count(listing.unresolved.len(), "unresolved Reference")
            )?;
        }
    }
    Ok(())
}

/// Writes every unresolved Reference with the Record that holds it.
pub(crate) fn unresolved(out: &mut impl Write, links: &[Link<'_, '_>]) -> io::Result<()> {
    let rows: Vec<Vec<String>> = links
        .iter()
        .map(|link| {
            let record = link.record.as_ref();
            vec![
                one_line(record.map_or("", |info| info.record_path)),
                one_line(record.map_or("", |info| info.name)),
                one_line(&link.location),
                guid(link),
            ]
        })
        .collect();
    table(out, &["record path", "name", "field", "GUID"], &rows)?;
    writeln!(out, "{}", count(links.len(), "unresolved Reference"))
}

/// Writes the edges of the file graph.
pub(crate) fn files(out: &mut impl Write, edges: &Edges<'_>) -> io::Result<()> {
    let rows: Vec<Vec<String>> = edges
        .edges
        .iter()
        .map(|(from, to)| vec![one_line(from), one_line(to)])
        .collect();
    table(out, &["record file", "refers to"], &rows)?;
    writeln!(
        out,
        "{} between {}",
        count(edges.edges.len(), "edge"),
        count(edges.files, "record file")
    )
}

fn heading(info: &Info<'_>) -> String {
    one_line(&format!(
        "{} ({}, {}, {})",
        info.record_path, info.name, info.type_name, info.guid
    ))
}

fn guid(link: &Link<'_, '_>) -> String {
    link.guid.clone().unwrap_or_default()
}

fn distinct(links: &[Link<'_, '_>]) -> usize {
    links
        .iter()
        .filter_map(|link| link.record.as_ref())
        .map(|info| info.guid.as_str())
        .collect::<HashSet<_>>()
        .len()
}

pub(crate) fn count(number: usize, noun: &str) -> String {
    let plural = if number == 1 { "" } else { "s" };
    format!("{number} {noun}{plural}")
}

pub(crate) fn table(out: &mut impl Write, header: &[&str], rows: &[Vec<String>]) -> io::Result<()> {
    table_rows(out, header, || rows.iter().map(Vec::as_slice))
}

pub(crate) fn table_rows<Row, Rows>(
    out: &mut impl Write,
    header: &[&str],
    rows: impl Fn() -> Rows,
) -> io::Result<()>
where
    Row: AsRef<[String]>,
    Rows: Iterator<Item = Row>,
{
    let mut widths: Vec<usize> = header.iter().map(|text| text.chars().count()).collect();
    for row in rows() {
        for (width, text) in widths.iter_mut().zip(row.as_ref()) {
            *width = (*width).max(text.chars().count());
        }
    }
    let head: Vec<String> = header.iter().map(|text| (*text).to_owned()).collect();
    let rule: Vec<String> = widths.iter().map(|width| "-".repeat(*width)).collect();
    line(out, &widths, &head)?;
    line(out, &widths, &rule)?;
    for row in rows() {
        line(out, &widths, row.as_ref())?;
    }
    Ok(())
}

fn line(out: &mut impl Write, widths: &[usize], cells: &[String]) -> io::Result<()> {
    let mut text = String::new();
    for (index, (width, cell)) in widths.iter().zip(cells).enumerate() {
        if index > 0 {
            text.push_str(GAP);
        }
        text.push_str(cell);
        text.extend(std::iter::repeat_n(
            ' ',
            width.saturating_sub(cell.chars().count()),
        ));
    }
    writeln!(out, "{}", text.trim_end())
}
