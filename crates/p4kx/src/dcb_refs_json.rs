// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io::{self, Write};
use std::path::Path;

use anyhow::Result;
use p4kx_extraction::{FieldOrigins, SourceInventory, SourceUseRole};
use serde::Serialize;

use crate::dcb_refs::{self, Info, Link, Listing, Mode};
use crate::forge_input::ForgeInput;
use crate::json;

#[path = "dcb_refs_file_json.rs"]
mod files;
#[path = "dcb_refs_origins.rs"]
mod origins;

use origins::{CountEvidence, HeaderField, Origins};

#[derive(Serialize)]
struct InfoJson<'a> {
    guid: &'a str,
    record_path: &'a str,
    name: &'a str,
    #[serde(rename = "type")]
    type_name: &'a str,
}

impl<'a> From<&'a Info<'_>> for InfoJson<'a> {
    fn from(info: &'a Info<'_>) -> Self {
        Self {
            guid: &info.guid,
            record_path: info.record_path,
            name: info.name,
            type_name: info.type_name,
        }
    }
}

#[derive(Serialize)]
struct LinkJson<'a> {
    record: Option<InfoJson<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guid: Option<&'a str>,
    location: &'a str,
    field: &'a str,
    indices: &'a [u32],
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
}

impl<'a> LinkJson<'a> {
    fn read(link: &'a Link<'_, '_>, origins: &Origins<'_, '_>) -> Result<Self> {
        Ok(Self {
            record: link.record.as_ref().map(InfoJson::from),
            guid: link.guid.as_deref(),
            location: &link.location,
            field: &link.field,
            indices: &link.indices,
            field_origins: origins.link(link)?,
        })
    }
}

#[derive(Serialize)]
struct ListingJson<'a> {
    record: InfoJson<'a>,
    reference_count: usize,
    record_count: usize,
    references: Vec<LinkJson<'a>>,
    unresolved_count: usize,
    unresolved: Vec<LinkJson<'a>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
}

impl<'a> ListingJson<'a> {
    fn read(
        listing: &'a Listing<'_, '_>,
        origins: &Origins<'_, '_>,
        whole_scan: Option<&CountEvidence>,
    ) -> Result<Self> {
        let counts = listing_counts(listing, origins, whole_scan)?;
        Ok(Self {
            record: InfoJson::from(&listing.target),
            reference_count: listing.references.len(),
            record_count: counts.records,
            references: links(&listing.references, origins)?,
            unresolved_count: listing.unresolved.len(),
            unresolved: links(&listing.unresolved, origins)?,
            field_origins: counts.fields,
        })
    }
}

struct ListingCounts {
    records: usize,
    fields: FieldOrigins,
}

fn listing_counts(
    listing: &Listing<'_, '_>,
    origins: &Origins<'_, '_>,
    whole_scan: Option<&CountEvidence>,
) -> Result<ListingCounts> {
    let mut fields = FieldOrigins::new();
    origins.record(&mut fields, &listing.target, None)?;
    let (mut references, mut unresolved) = if let Some(scan) = whole_scan {
        let mut references = scan.clone();
        references.add(origins.header(
            listing.target.original().index,
            HeaderField::Guid,
            SourceUseRole::Selector,
        )?)?;
        (references, CountEvidence::empty())
    } else {
        let scan = origins.record_scan(listing.target.original().index)?;
        (scan.clone(), scan)
    };
    retain_links(&mut references, &listing.references, origins)?;
    references.field(origins, &mut fields, "/reference_count")?;
    let mut records = HashSet::new();
    for record in listing
        .references
        .iter()
        .filter_map(|link| link.record.as_ref())
    {
        records.insert(record.guid.as_str());
        references.add(origins.header(
            record.original().index,
            HeaderField::Guid,
            SourceUseRole::Selector,
        )?)?;
    }
    references.field(origins, &mut fields, "/record_count")?;
    retain_links(&mut unresolved, &listing.unresolved, origins)?;
    unresolved.field(origins, &mut fields, "/unresolved_count")?;
    Ok(ListingCounts {
        records: records.len(),
        fields,
    })
}

fn retain_links(
    evidence: &mut CountEvidence,
    links: &[Link<'_, '_>],
    origins: &Origins<'_, '_>,
) -> Result<()> {
    for link in links {
        evidence.add(origins.relation(link.original())?)?;
    }
    Ok(())
}

fn links<'a>(items: &'a [Link<'_, '_>], origins: &Origins<'_, '_>) -> Result<Vec<LinkJson<'a>>> {
    items
        .iter()
        .map(|link| LinkJson::read(link, origins))
        .collect()
}

#[derive(Serialize)]
struct ListingsJson<'a> {
    archive: String,
    direction: &'static str,
    records: Vec<ListingJson<'a>>,
    provenance: SourceInventory,
}

/// Writes the selected Reference JSON view with its original field evidence.
pub(crate) fn write(
    out: &mut impl Write,
    input: &ForgeInput,
    target: Option<&str>,
    mode: Mode,
) -> Result<()> {
    origins::read(input, target, |origins, targets| {
        let index = origins.index();
        match mode {
            Mode::Incoming | Mode::Outgoing => {
                let rows = dcb_refs::listings(index.forge(), index.index(), targets, mode);
                listings(out, input.path(), mode, &rows, origins)
            }
            Mode::Files => files::write(out, input.path(), targets, origins),
            Mode::Unresolved => {
                let rows = dcb_refs::unresolved(index.forge(), index.index());
                unresolved(out, input.path(), &rows, origins)
            }
        }
    })
}

fn listings(
    out: &mut impl Write,
    archive: &Path,
    mode: Mode,
    listings: &[Listing<'_, '_>],
    origins: &Origins<'_, '_>,
) -> Result<()> {
    let (direction, scan) = if mode == Mode::Incoming {
        ("incoming", Some(origins.whole_scan()?))
    } else {
        ("outgoing", None)
    };
    let records = listings
        .iter()
        .map(|listing| ListingJson::read(listing, origins, scan.as_ref()))
        .collect::<Result<Vec<_>>>()?;
    Ok(line(
        out,
        &ListingsJson {
            archive: json::path(archive),
            direction,
            records,
            provenance: origins.inventory()?,
        },
    )?)
}

#[derive(Serialize)]
struct UnresolvedJson<'a> {
    archive: String,
    count: usize,
    unresolved: Vec<LinkJson<'a>>,
    #[serde(rename = "fieldOrigins")]
    field_origins: FieldOrigins,
    provenance: SourceInventory,
}

fn unresolved(
    out: &mut impl Write,
    archive: &Path,
    items: &[Link<'_, '_>],
    origins: &Origins<'_, '_>,
) -> Result<()> {
    let mut count = origins.whole_scan()?;
    retain_links(&mut count, items, origins)?;
    let mut field_origins = FieldOrigins::new();
    count.field(origins, &mut field_origins, "/count")?;
    Ok(line(
        out,
        &UnresolvedJson {
            archive: json::path(archive),
            count: items.len(),
            unresolved: links(items, origins)?,
            field_origins,
            provenance: origins.inventory()?,
        },
    )?)
}

fn line(out: &mut impl Write, value: &impl Serialize) -> io::Result<()> {
    serde_json::to_writer(&mut *out, value)?;
    writeln!(out)
}
