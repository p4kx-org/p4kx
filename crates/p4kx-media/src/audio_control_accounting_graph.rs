// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, VecDeque};
use std::io;

use p4kx_extraction::Archive;
use serde_json::Value;

use crate::MediaRecord;
use crate::audio_control_graph::rows::{CANDIDATE, RESOLUTION};
use crate::audio_control_payload::REFERENCE;
use crate::audio_control_reference::paths::PATH_CANDIDATE;

use super::super::Census;
use super::{invalid, require};

#[derive(Default)]
pub(super) struct Links {
    path: Option<PathCandidates>,
    named: VecDeque<Site>,
    next_query: u64,
    next_resolution: u64,
    candidates: Option<Candidates>,
}

struct Candidates {
    query: u64,
    next: u64,
    count: u64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
struct Site {
    entry: usize,
    element: u64,
    declaration: u64,
}

struct PathCandidates {
    site: Site,
    target: String,
    count: usize,
}

impl Links {
    pub(super) fn record(
        &mut self,
        row: &MediaRecord,
        entry: Option<usize>,
        archive: &Archive,
        counts: &BTreeMap<usize, Census>,
    ) -> io::Result<()> {
        let payload = row
            .data
            .get("payload")
            .ok_or_else(|| invalid("ATL link has no payload"))?;
        match row.subresource.namespace.as_str() {
            PATH_CANDIDATE => self.path_candidate(row, payload, entry, archive),
            REFERENCE => self.authored(row, payload, entry),
            RESOLUTION => self.resolution(row, payload),
            CANDIDATE => self.candidate(row, payload, counts),
            _ => Err(invalid("unexpected ATL link namespace")),
        }
    }

    fn path_candidate(
        &mut self,
        row: &MediaRecord,
        payload: &Value,
        entry: Option<usize>,
        archive: &Archive,
    ) -> io::Result<()> {
        let [element, declaration, ordinal] = row.subresource.indices.as_slice() else {
            return Err(invalid("ATL path candidate position"));
        };
        let site = site(entry, *element, *declaration)?;
        let target = physical(payload, "target_entry_index")?;
        let target = archive
            .entries()
            .get(target)
            .ok_or_else(|| invalid("ATL path candidate physical Entry"))?;
        require(
            payload.get("key_required").and_then(Value::as_bool)
                == Some(target.is_encrypted() && !archive.has_key()),
            "ATL path candidate key state",
        )?;
        let path = normal_path(target.path());
        let pending = self.path.get_or_insert_with(|| PathCandidates {
            site,
            target: path.clone(),
            count: 0,
        });
        require(
            pending.site == site
                && pending.target == path
                && u64::try_from(pending.count).ok() == Some(*ordinal),
            "ATL path candidates change owner, path, or order",
        )?;
        pending.count += 1;
        Ok(())
    }

    fn authored(
        &mut self,
        row: &MediaRecord,
        payload: &Value,
        entry: Option<usize>,
    ) -> io::Result<()> {
        let [element, declaration] = row.subresource.indices.as_slice() else {
            return Err(invalid("ATL reference occurrence position"));
        };
        let site = site(entry, *element, *declaration)?;
        let link = payload
            .get("link")
            .ok_or_else(|| invalid("ATL reference has no link status"))?;
        let path = self.path.take();
        match link.get("status").and_then(Value::as_str) {
            Some("path-search") => check_path(path, &site, payload, link),
            Some("scoped-search") => {
                require(
                    path.is_none() && decimal(link, "query_index")? == self.next_query,
                    "ATL named query ordering",
                )?;
                self.named.push_back(site);
                self.next_query += 1;
                Ok(())
            }
            Some("unavailable" | "not-selected") => {
                require(path.is_none(), "orphan ATL path candidate")
            }
            _ => Err(invalid(
                "unexpected ATL reference status for audio-control-only projection",
            )),
        }
    }

    fn resolution(&mut self, row: &MediaRecord, payload: &Value) -> io::Result<()> {
        self.no_candidate_pending()?;
        let [query] = row.subresource.indices.as_slice() else {
            return Err(invalid("ATL resolution position"));
        };
        let original = self
            .named
            .pop_front()
            .ok_or_else(|| invalid("ATL resolution has no declared named query"))?;
        let observed = site(
            Some(physical(payload, "owner_entry_index")?),
            decimal(payload, "owner_element_index")?,
            decimal(payload, "owner_declaration_ordinal")?,
        )?;
        let count = decimal(payload, "observed_candidates")?;
        let complete = payload
            .get("candidates_complete")
            .and_then(Value::as_bool)
            .ok_or_else(|| invalid("ATL candidate completeness missing"))?;
        let resolution = match (complete, count) {
            (false, _) => "unavailable",
            (true, 0) => "missing",
            (true, 1) => "resolved",
            (true, _) => "ambiguous",
        };
        require(
            *query == self.next_resolution
                && original == observed
                && payload.get("resolution").and_then(Value::as_str) == Some(resolution),
            "ATL named resolution disagrees with its declaration and candidate state",
        )?;
        self.candidates = (count != 0).then_some(Candidates {
            query: *query,
            next: 0,
            count,
        });
        self.next_resolution += 1;
        Ok(())
    }

    fn candidate(
        &mut self,
        row: &MediaRecord,
        payload: &Value,
        counts: &BTreeMap<usize, Census>,
    ) -> io::Result<()> {
        let [query, ordinal] = row.subresource.indices.as_slice() else {
            return Err(invalid("ATL named candidate position"));
        };
        let pending = self
            .candidates
            .as_mut()
            .ok_or_else(|| invalid("ATL named candidate has no resolution"))?;
        let target = physical(payload, "target_entry_index")?;
        decimal(payload, "target_element_index")?;
        decimal(payload, "target_declaration_ordinal")?;
        require(
            pending.query == *query
                && pending.next == *ordinal
                && *ordinal < pending.count
                && counts
                    .get(&target)
                    .is_some_and(|source| source.declarations > 0),
            "ATL named candidate cannot be joined to its original source",
        )?;
        let next = ordinal
            .checked_add(1)
            .ok_or_else(|| invalid("ATL candidate position overflow"))?;
        pending.next = next;
        if next == pending.count {
            self.candidates = None;
        }
        Ok(())
    }

    pub(super) fn no_candidate_pending(&self) -> io::Result<()> {
        require(
            self.path.is_none() && self.candidates.is_none(),
            "ATL candidate stream is incomplete",
        )
    }

    pub(super) fn finish(&self) -> io::Result<()> {
        self.no_candidate_pending()?;
        require(
            self.named.is_empty(),
            "ATL named reference lacks its resolution row",
        )
    }
}

fn check_path(
    path: Option<PathCandidates>,
    site: &Site,
    payload: &Value,
    link: &Value,
) -> io::Result<()> {
    let count = physical(link, "observed_candidates")?;
    require(
        match path {
            Some(path) => {
                path.site == *site
                    && path.count == count
                    && payload
                        .pointer("/target/path")
                        .and_then(Value::as_str)
                        .is_some_and(|name| normal_path(name) == path.target)
            }
            None => count == 0,
        },
        "ATL path reference disagrees with its physical candidates",
    )
}

fn normal_path(path: &str) -> String {
    path.replace('\\', "/").to_ascii_lowercase()
}

fn decimal(row: &Value, key: &str) -> io::Result<u64> {
    row.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| invalid("ATL graph coordinate is missing"))
        .and_then(|value| crate::origin_decimal::parse(value).map_err(invalid))
}

fn physical(row: &Value, key: &str) -> io::Result<usize> {
    usize::try_from(decimal(row, key)?).map_err(io::Error::other)
}

fn site(entry: Option<usize>, element: u64, declaration: u64) -> io::Result<Site> {
    Ok(Site {
        entry: entry.ok_or_else(|| invalid("ATL declaration has no physical owner"))?,
        element,
        declaration,
    })
}
