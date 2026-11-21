// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{Guid, Tag, TagTree, TagTreeIssues};
use thiserror::Error;

use crate::cli::{DcbTagsArgs, TagsFormat};
use crate::dcb_refs::JsonWithTable;
use crate::forge_input::ForgeInput;
use crate::{dcb_tags_json, dcb_tags_table, exit};

#[path = "dcb_tags_origins.rs"]
pub(crate) mod origins;

#[derive(Debug, Clone, Copy)]
pub(crate) enum Selection {
    All,
    Guid,
    Path,
}

/// The text names no tag.
#[derive(Debug, Error)]
#[error("no tag has the tag path or GUID `{0}`")]
pub(crate) struct UnknownTag(String);

/// One tag of a listing and how far it is indented below the first tag shown.
#[derive(Debug, Clone, Copy)]
pub(crate) struct Row<'t> {
    pub(crate) tag: Tag<'t>,
    pub(crate) indent: usize,
}

/// The tags `p4kx dcb tags` shows: the tags asked for, each followed by everything below it.
#[derive(Debug)]
pub(crate) struct Listing<'t> {
    pub(crate) tops: Vec<Tag<'t>>,
    pub(crate) rows: Vec<Row<'t>>,
    pub(crate) total: usize,
    pub(crate) issues: TagTreeIssues,
    pub(crate) selection: Selection,
}

/// Lists the tag tree of the Archive's DataForge as the arguments ask and writes it to stdout.
pub(crate) fn run(args: &DcbTagsArgs, as_json: bool) -> Result<ExitCode> {
    let json = match (as_json, args.format) {
        (true, Some(TagsFormat::Table)) => return Err(JsonWithTable.into()),
        (as_json, format) => as_json || format == Some(TagsFormat::Json),
    };
    let input = ForgeInput::open(&args.archive)?;
    let origins = json
        .then(|| origins::TagOrigins::build(&input))
        .transpose()?;
    let table_tags;
    let tags = if let Some(origins) = &origins {
        origins.tree()
    } else {
        let forge = input.entry().open()?;
        table_tags = TagTree::build(&forge);
        &table_tags
    };
    warn(tags.issues());
    let listing = listing(tags, args.tag.as_deref())?;
    let mut out = BufWriter::new(io::stdout().lock());
    let written = if let Some(origins) = &origins {
        dcb_tags_json::write(&mut out, &args.archive, &listing, args.tree, origins)
    } else {
        dcb_tags_table::write(&mut out, &listing, args.tree)
    };
    match written.and_then(|()| out.flush()) {
        Err(error) if error.kind() != io::ErrorKind::BrokenPipe => Err(error.into()),
        _ => Ok(ExitCode::from(exit::SUCCESS)),
    }
}

/// Returns the tags `text` names, each with everything below it, or every tag without `text`.
fn listing<'t>(tags: &'t TagTree, text: Option<&str>) -> Result<Listing<'t>, UnknownTag> {
    let (tops, selection) = match text {
        None => (tags.roots().collect(), Selection::All),
        Some(text) => {
            let (found, selection) = text.parse::<Guid>().map_or_else(
                |_| (tags.find_path(text).collect(), Selection::Path),
                |guid| {
                    (
                        tags.get(guid).into_iter().collect::<Vec<_>>(),
                        Selection::Guid,
                    )
                },
            );
            if found.is_empty() {
                return Err(UnknownTag(text.to_owned()));
            }
            (found, selection)
        }
    };
    let rows = tops
        .iter()
        .flat_map(|top| {
            let first = top.depth();
            std::iter::once(*top)
                .chain(top.descendants())
                .map(move |tag| Row {
                    tag,
                    indent: tag.depth().saturating_sub(first),
                })
        })
        .collect();
    Ok(Listing {
        tops,
        rows,
        total: tags.len(),
        issues: tags.issues(),
        selection,
    })
}

fn warn(issues: TagTreeIssues) {
    let mut err = io::stderr().lock();
    let notes = [
        (
            issues.unclassified(),
            "Records cannot be classified because their type metadata is unreadable",
        ),
        (
            issues.unreadable(),
            "Tag Records cannot be read and are left out",
        ),
        (issues.dangling(), "listed tags are no Tag Record"),
        (
            issues.repeated(),
            "tags are listed more than once; the first listing counts",
        ),
        (
            issues.duplicate(),
            "Tag Records repeat the GUID of an earlier one and are left out",
        ),
        (
            issues.unlisted(),
            "tags are not listed by the TagDatabase and stand as roots of their own",
        ),
    ];
    for (count, note) in notes {
        if count > 0 {
            let _unreported = writeln!(err, "warning: {count} {note}");
        }
    }
}
