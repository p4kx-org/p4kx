// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

use anyhow::Result;
use p4kx_extraction::{DataForge, DataForgeError, IndexedReference, ReferenceIndex, SourceUseRole};
use thiserror::Error;

use crate::cli::{DcbRefsArgs, RefsFormat};
use crate::forge_input::ForgeInput;
use crate::{dcb_refs_json, dcb_refs_table, exit};

/// The text names no Record.
#[derive(Debug, Error)]
#[error("no Record has the GUID or record path `{0}`")]
pub(crate) struct UnknownRecord(String);

/// `--json` with `--format table`.
#[derive(Debug, Error)]
#[error("--json writes JSON: leave --format out or use --format json")]
pub(crate) struct JsonWithTable;

/// What `p4kx dcb refs` lists.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Mode {
    Incoming,
    Outgoing,
    Files,
    Unresolved,
}

/// A Record as the listings name it.
#[derive(Debug, Clone)]
pub(crate) struct Info<'f> {
    pub(crate) guid: String,
    pub(crate) record_path: &'f str,
    pub(crate) name: &'f str,
    pub(crate) type_name: &'f str,
    original: HeaderReads,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct HeaderReads {
    pub(crate) index: u32,
    pub(crate) path: SourceUseRole,
    pub(crate) name: SourceUseRole,
    pub(crate) source_type: SourceUseRole,
}

impl Info<'_> {
    pub(crate) const fn original(&self) -> HeaderReads {
        self.original
    }
}

/// One Reference of a listing.
#[derive(Debug, Clone)]
pub(crate) struct Link<'index, 'f> {
    pub(crate) record: Option<Info<'f>>,
    pub(crate) guid: Option<String>,
    pub(crate) location: String,
    pub(crate) field: String,
    pub(crate) indices: Vec<u32>,
    original: IndexedReference<'index>,
}

impl<'index> Link<'index, '_> {
    pub(crate) const fn original(&self) -> IndexedReference<'index> {
        self.original
    }
}

/// The References to or from one Record.
#[derive(Debug)]
pub(crate) struct Listing<'index, 'f> {
    pub(crate) target: Info<'f>,
    pub(crate) references: Vec<Link<'index, 'f>>,
    pub(crate) unresolved: Vec<Link<'index, 'f>>,
}

/// The edges between record files, as pairs of record paths.
#[derive(Debug)]
pub(crate) struct Edges<'f> {
    pub(crate) files: usize,
    pub(crate) edges: Vec<(&'f str, &'f str)>,
}

impl DcbRefsArgs {
    /// Returns what the arguments ask for.
    const fn mode(&self) -> Mode {
        if self.incoming {
            Mode::Incoming
        } else if self.outgoing {
            Mode::Outgoing
        } else if self.files {
            Mode::Files
        } else {
            Mode::Unresolved
        }
    }
}

/// Lists the References of the Archive's DataForge as the arguments ask and writes the listing
/// to stdout.
pub(crate) fn run(args: &DcbRefsArgs, as_json: bool) -> Result<ExitCode> {
    let json = match (as_json, args.format) {
        (true, Some(RefsFormat::Table)) => return Err(JsonWithTable.into()),
        (as_json, format) => as_json || format == Some(RefsFormat::Json),
    };
    let input = ForgeInput::open(&args.archive)?;
    let mut out = BufWriter::new(io::stdout().lock());
    let target = args.target.as_deref();
    let written = if json {
        dcb_refs_json::write(&mut out, &input, target, args.mode())
    } else {
        let forge = input.entry().open()?;
        let targets = select(&forge, target)?;
        let index = ReferenceIndex::build(&forge);
        warn(&index);
        write_table(&mut out, &forge, &index, &targets, args.mode()).map_err(Into::into)
    };
    match written.and_then(|()| out.flush().map_err(Into::into)) {
        Err(error)
            if error
                .downcast_ref::<io::Error>()
                .is_none_or(|source| source.kind() != io::ErrorKind::BrokenPipe) =>
        {
            Err(error)
        }
        _ => Ok(ExitCode::from(exit::SUCCESS)),
    }
}

fn write_table(
    out: &mut impl Write,
    forge: &DataForge<'_>,
    index: &ReferenceIndex,
    targets: &[u32],
    mode: Mode,
) -> io::Result<()> {
    match mode {
        Mode::Incoming | Mode::Outgoing => {
            dcb_refs_table::listings(out, mode, &listings(forge, index, targets, mode))
        }
        Mode::Files => dcb_refs_table::files(out, &edges(forge, index, targets)),
        Mode::Unresolved => dcb_refs_table::unresolved(out, &unresolved(forge, index)),
    }
}

/// Returns the original Record indices named by the requested GUID or record path.
pub(crate) fn select(
    forge: &DataForge<'_>,
    target: Option<&str>,
) -> Result<Vec<u32>, UnknownRecord> {
    let Some(text) = target else {
        return Ok(Vec::new());
    };
    let found = forge.records_named(text);
    if found.is_empty() {
        return Err(UnknownRecord(text.to_owned()));
    }
    Ok(found)
}

pub(crate) fn warn(index: &ReferenceIndex) {
    let mut err = io::stderr().lock();
    if index.is_truncated() {
        let _unreported = writeln!(
            err,
            "warning: the index contains only the first {} References",
            index.len()
        );
    }
    if index.unreadable_records() > 0 {
        let _unreported = writeln!(
            err,
            "warning: {} Records cannot be read and are not indexed",
            index.unreadable_records()
        );
    }
}

fn info<'source>(forge: &'source DataForge<'source>, index: u32) -> Option<Info<'source>> {
    let record = forge.record(index)?;
    let (record_path, path) = display(record.record_path(), SourceUseRole::Conversion);
    let (name, name_role) = record
        .stored_name()
        .map_or(("", SourceUseRole::Attempted), |name| {
            name.fold(
                || ("", SourceUseRole::Conversion),
                || ("", SourceUseRole::Conversion),
                |text| (text, SourceUseRole::Payload),
            )
        });
    let (type_name, source_type) = display(record.type_name(), SourceUseRole::Payload);
    Some(Info {
        guid: record.guid().to_string(),
        record_path,
        name,
        type_name,
        original: HeaderReads {
            index: record.index(),
            path,
            name: name_role,
            source_type,
        },
    })
}

fn display(value: Result<&str, DataForgeError>, role: SourceUseRole) -> (&str, SourceUseRole) {
    value.map_or(("", SourceUseRole::Attempted), |value| (value, role))
}

fn link<'index, 'source>(
    forge: &'source DataForge<'source>,
    reference: IndexedReference<'index>,
    other: Option<u32>,
) -> Link<'index, 'source> {
    Link {
        record: other.and_then(|record| info(forge, record)),
        guid: reference.unresolved_guid().map(|guid| guid.to_string()),
        location: reference.location(),
        field: reference.field_path().to_owned(),
        indices: reference.indices().to_vec(),
        original: reference,
    }
}

pub(crate) fn listings<'index, 'source>(
    forge: &'source DataForge<'source>,
    index: &'index ReferenceIndex,
    targets: &[u32],
    mode: Mode,
) -> Vec<Listing<'index, 'source>> {
    targets
        .iter()
        .filter_map(|&target| {
            let mut listing = Listing {
                target: info(forge, target)?,
                references: Vec::new(),
                unresolved: Vec::new(),
            };
            if mode == Mode::Incoming {
                for reference in index.incoming(target) {
                    listing
                        .references
                        .push(link(forge, reference, Some(reference.source())));
                }
            } else {
                for reference in index.outgoing(target) {
                    match reference.target() {
                        Some(other) => {
                            listing.references.push(link(forge, reference, Some(other)));
                        }
                        None => listing.unresolved.push(link(forge, reference, None)),
                    }
                }
            }
            Some(listing)
        })
        .collect()
}

pub(crate) fn unresolved<'index, 'source>(
    forge: &'source DataForge<'source>,
    index: &'index ReferenceIndex,
) -> Vec<Link<'index, 'source>> {
    index
        .unresolved()
        .map(|reference| link(forge, reference, Some(reference.source())))
        .collect()
}

/// Returns the edges of the file graph; with `targets`, only those from or to their files.
fn edges<'f>(forge: &'f DataForge<'f>, index: &ReferenceIndex, targets: &[u32]) -> Edges<'f> {
    let graph = index.file_graph(forge);
    let wanted: Vec<u32> = targets
        .iter()
        .filter_map(|&target| {
            let path = forge.record(target)?.record_path().ok()?;
            graph.position(path)
        })
        .collect();
    let paths = graph.paths();
    let edges = graph
        .edges()
        .iter()
        .filter(|(from, to)| targets.is_empty() || wanted.contains(from) || wanted.contains(to))
        .filter_map(|(from, to)| {
            let path = |file: &u32| paths.get(*file as usize).copied();
            Some((path(from)?, path(to)?))
        })
        .collect();
    Edges {
        files: paths.len(),
        edges,
    }
}
