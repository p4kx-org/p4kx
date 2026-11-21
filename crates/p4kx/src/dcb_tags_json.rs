// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, Write};
use std::path::Path;

use p4kx_extraction::{Tag, TagPath, TagTreeIssues};
use serde::ser::{Serialize, SerializeMap, Serializer};

use crate::dcb_tags::Listing;
use crate::dcb_tags::origins::TagOrigins;
use crate::json;

struct PathText<'tree>(TagPath<'tree>);

impl Serialize for PathText<'_> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(&self.0)
    }
}

struct Issues(TagTreeIssues);

impl Serialize for Issues {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(8))?;
        map.serialize_entry("unclassified", &self.0.unclassified())?;
        map.serialize_entry("unreadable", &self.0.unreadable())?;
        map.serialize_entry("dangling", &self.0.dangling())?;
        map.serialize_entry("repeated", &self.0.repeated())?;
        map.serialize_entry("too_deep", &0)?;
        map.serialize_entry("too_long", &0)?;
        map.serialize_entry("duplicate", &self.0.duplicate())?;
        map.serialize_entry("unlisted", &self.0.unlisted())?;
        map.end()
    }
}

/// Writes the listing as one line of JSON.
pub(crate) fn write(
    out: &mut impl Write,
    archive: &Path,
    listing: &Listing<'_>,
    tree: bool,
    origins: &TagOrigins,
) -> io::Result<()> {
    let summary = origins.summary(listing).map_err(io::Error::other)?;
    out.write_all(b"{\"archive\":")?;
    serde_json::to_writer(&mut *out, &json::path(archive))?;
    field(out, "tag_count", &listing.total)?;
    field(out, "shown_count", &listing.rows.len())?;
    field(out, "issues", &Issues(listing.issues))?;
    field(out, "tree", &tree)?;
    field(out, "fieldOrigins", &summary)?;
    field(out, "provenance", origins.inventory())?;
    out.write_all(b",\"tags\":[")?;
    if tree {
        for (index, top) in listing.tops.iter().enumerate() {
            comma(out, index)?;
            nested(out, *top, origins)?;
        }
    } else {
        for (index, row) in listing.rows.iter().enumerate() {
            comma(out, index)?;
            flat(out, row.tag, origins)?;
        }
    }
    out.write_all(b"]}\n")
}

fn head(out: &mut impl Write, tag: Tag<'_>) -> io::Result<()> {
    out.write_all(b"{\"guid\":")?;
    serde_json::to_writer(&mut *out, &tag.guid().to_string())?;
    field(out, "name", &tag.name())?;
    field(out, "path", &PathText(tag.path_view()))?;
    field(out, "legacy_id", &tag.legacy_id())
}

fn field<T: Serialize>(out: &mut impl Write, name: &str, value: &T) -> io::Result<()> {
    out.write_all(b",")?;
    serde_json::to_writer(&mut *out, name)?;
    out.write_all(b":")?;
    serde_json::to_writer(out, value)?;
    Ok(())
}

fn comma(out: &mut impl Write, index: usize) -> io::Result<()> {
    if index != 0 {
        out.write_all(b",")?;
    }
    Ok(())
}

fn flat(out: &mut impl Write, tag: Tag<'_>, origins: &TagOrigins) -> io::Result<()> {
    let fields = origins
        .row(tag.position(), true)
        .map_err(io::Error::other)?;
    head(out, tag)?;
    field(
        out,
        "parent",
        &tag.parent().map(|parent| parent.guid().to_string()),
    )?;
    field(out, "depth", &tag.depth())?;
    field(out, "fieldOrigins", &fields)?;
    out.write_all(b",\"children\":[")?;
    for (index, child) in tag.children().enumerate() {
        comma(out, index)?;
        serde_json::to_writer(&mut *out, &child.guid().to_string())?;
    }
    out.write_all(b"]}")
}

fn nested(out: &mut impl Write, top: Tag<'_>, origins: &TagOrigins) -> io::Result<()> {
    let base_depth = top.depth();
    let mut open = 0;
    for tag in std::iter::once(top).chain(top.descendants()) {
        let depth = tag.depth() - base_depth;
        if open > depth {
            close(out, open - depth)?;
            out.write_all(b",")?;
        }
        let fields = origins
            .row(tag.position(), false)
            .map_err(io::Error::other)?;
        head(out, tag)?;
        field(out, "fieldOrigins", &fields)?;
        out.write_all(b",\"children\":[")?;
        open = depth + 1;
    }
    close(out, open)
}

fn close(out: &mut impl Write, count: usize) -> io::Result<()> {
    for _ in 0..count {
        out.write_all(b"]}")?;
    }
    Ok(())
}
