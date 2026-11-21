// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::io;

use p4kx_extraction::{Archive, Checksum};
use sha2::{Digest as _, Sha256};

use crate::artifact::StreamObserver;
use crate::audio_control_graph::rows::{CANDIDATE, RESOLUTION};
use crate::audio_control_payload as wire;
use crate::audio_control_reference::paths::PATH_CANDIDATE;
use crate::{Algorithm, ByteScope, ContainerIdentity, Identity, MediaKind, MediaRecord, Status};

use super::{Census, TestResult};

#[path = "audio_control_accounting_graph.rs"]
mod graph;
#[path = "audio_control_accounting_origins.rs"]
mod origins;

const RECORDS: &str = "audio-control.jsonl";
const FIELDS: &str = "field-origins.jsonl";
const SOURCES: &str = "origin-sources.jsonl";
const USES: &str = "origin-uses.jsonl";

pub(super) struct Observer<'a> {
    archive: &'a Archive,
    originals: &'a BTreeMap<usize, String>,
    state: RefCell<State>,
}

#[derive(Default)]
struct State {
    record: Vec<u8>,
    field: Vec<u8>,
    source: Vec<u8>,
    used: Vec<u8>,
    counts: BTreeMap<usize, Census>,
    origins: origins::Origins,
    links: graph::Links,
}

impl<'a> Observer<'a> {
    pub(super) fn new(archive: &'a Archive, originals: &'a BTreeMap<usize, String>) -> Self {
        Self {
            archive,
            originals,
            state: RefCell::new(State::default()),
        }
    }

    pub(super) fn counts(self) -> TestResult<BTreeMap<usize, Census>> {
        let state = self.state.into_inner();
        state.origins.finish()?;
        state.links.finish()?;
        Ok(state.counts)
    }
}

impl StreamObserver for Observer<'_> {
    fn write(&self, relative: &str, bytes: &[u8]) -> io::Result<()> {
        let mut guard = self.state.try_borrow_mut().map_err(io::Error::other)?;
        let state = &mut *guard;
        match relative {
            RECORDS => lines(&mut state.record, bytes, |line| {
                record(
                    line,
                    self.archive,
                    self.originals,
                    &mut state.counts,
                    &mut state.origins,
                    &mut state.links,
                )
            }),
            FIELDS => lines(&mut state.field, bytes, |line| state.origins.row(line)),
            SOURCES => lines(&mut state.source, bytes, |line| {
                state.origins.source(line, self.originals)
            }),
            USES => lines(&mut state.used, bytes, |line| state.origins.use_row(line)),
            _ => Ok(()),
        }
    }

    fn finish(&self, relative: &str) -> io::Result<()> {
        let state = self.state.try_borrow().map_err(io::Error::other)?;
        let complete = match relative {
            RECORDS => state.record.is_empty(),
            FIELDS => state.field.is_empty(),
            SOURCES => state.source.is_empty(),
            USES => state.used.is_empty(),
            _ => true,
        };
        require(complete, "unterminated Media observation line")
    }
}

fn lines(
    pending: &mut Vec<u8>,
    bytes: &[u8],
    mut line: impl FnMut(&[u8]) -> io::Result<()>,
) -> io::Result<()> {
    for piece in bytes.split_inclusive(|byte| *byte == b'\n') {
        if piece.last() != Some(&b'\n') {
            pending.extend_from_slice(piece);
        } else if pending.is_empty() {
            line(piece)?;
        } else {
            pending.extend_from_slice(piece);
            line(pending)?;
            pending.clear();
        }
    }
    Ok(())
}

fn record(
    line: &[u8],
    archive: &Archive,
    originals: &BTreeMap<usize, String>,
    counts: &mut BTreeMap<usize, Census>,
    origins: &mut origins::Origins,
    links: &mut graph::Links,
) -> io::Result<()> {
    let row: MediaRecord = serde_json::from_slice(line)?;
    require(
        row.kind == MediaKind::AudioControl && row.status == Status::Complete,
        "non-complete ATL Media record",
    )?;
    let namespace = &row.subresource.namespace;
    let scope = match &row.source.identity {
        Identity::Entry { containers, entry }
            if !matches!(namespace.as_str(), RESOLUTION | CANDIDATE) =>
        {
            require(containers.is_empty(), "non-root ATL source")?;
            let position = entry.entry_index;
            let raw = row
                .source
                .raw
                .as_ref()
                .ok_or_else(|| invalid("missing ATL raw receipt"))?;
            let physical = archive
                .entries()
                .get(position)
                .ok_or_else(|| invalid("unknown ATL Entry"))?;
            require(
                row.source.size == physical.uncompressed_size()
                    && raw.size == row.source.size
                    && raw.digest.algorithm == Algorithm::Sha256
                    && raw.digest.scope == ByteScope::DecodedEntryPayload
                    && originals.get(&position) == Some(&raw.digest.hex),
                "ATL raw receipt disagrees with original verified bytes",
            )?;
            let census = counts.entry(position).or_default();
            if namespace == wire::DOCUMENT {
                require(
                    *entry
                        == ContainerIdentity::try_new_with_entry(archive, physical)
                            .map_err(io::Error::other)?,
                    "ATL record changed physical source identity",
                )?;
            }
            census.observe(namespace);
            Some(position)
        }
        Identity::Root if matches!(namespace.as_str(), RESOLUTION | CANDIDATE) => None,
        _ => return Err(invalid("unexpected physical source for ATL record")),
    };
    origins.record(line, &row, scope, archive)?;
    match namespace.as_str() {
        PATH_CANDIDATE | wire::REFERENCE | RESOLUTION | CANDIDATE => {
            links.record(&row, scope, archive, counts)
        }
        _ => links.no_candidate_pending(),
    }
}

fn sha256(bytes: &[u8]) -> String {
    Checksum::Sha256(Sha256::digest(bytes).into()).hex()
}

fn invalid(message: &'static str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, message)
}

fn require(condition: bool, message: &'static str) -> io::Result<()> {
    if condition {
        Ok(())
    } else {
        Err(invalid(message))
    }
}
