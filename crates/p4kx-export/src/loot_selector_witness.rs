// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::error::Error;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use p4kx_assembly::ChildContainerPath;
use p4kx_extraction::{Archive, ArchiveWatch, Entry};
use serde::Deserialize;

use super::selector_shape::{self, Shape};
use crate::CatalogError;
use crate::catalog_read::original::{EntryCapture, EntryRead, EntryXml, with_entry_capture};
use crate::entry_source::EntrySource;
use crate::export_entry_ledger::EntryOperation;
use crate::starmap_index::{self, Index};

type TestResult<T = ()> = Result<T, Box<dyn Error>>;
type MemberRequests<'source> = BTreeMap<usize, BTreeMap<usize, BTreeSet<&'source str>>>;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Survey {
    build: String,
    source_rows: Vec<SourceRow>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SourceRow {
    kind: String,
    role: String,
    selector: String,
    authoring_entry_routes: Vec<Vec<usize>>,
    same_basename_root_candidates: usize,
}

impl Survey {
    fn load(path: &Path) -> TestResult<Self> {
        let file = std::fs::File::open(path).map_err(|_| "open the private selector receipt")?;
        serde_json::from_reader(file).map_err(|_| "invalid private selector receipt".into())
    }

    fn requests(&self) -> (usize, usize, MemberRequests<'_>) {
        let mut requested = MemberRequests::new();
        let mut rows = 0;
        let mut unresolved = 0;
        for row in self.source_rows.iter().filter(|row| {
            row.kind == "lootContainers"
                && row.role == "derivedInput"
                && row.same_basename_root_candidates > 0
        }) {
            rows += 1;
            if row.authoring_entry_routes.is_empty() {
                unresolved += 1;
            }
            for route in &row.authoring_entry_routes {
                if let [owner, member] = route.as_slice() {
                    requested
                        .entry(*owner)
                        .or_default()
                        .entry(*member)
                        .or_default()
                        .insert(&row.selector);
                } else {
                    unresolved += 1;
                }
            }
        }
        (rows, unresolved, requested)
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
enum State {
    OwnerUnavailable,
    MemberUnavailable,
    ParseUnavailable,
    DifferentSelection,
    UnboundSelection,
    Observed(Observation),
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct Observation {
    shape: Shape,
    current_outer_matches: usize,
    permitted_relative: bool,
    exact_containing_matches: usize,
    owner_directory_leads: usize,
    root_basename_leads: usize,
}

#[test]
#[ignore = "installed Archive and private P4KX_LOOT_SELECTOR_WITNESS receipt"]
fn installed_loot_selector_source_scopes() -> TestResult {
    let selection = std::env::var_os("P4KX_LOOT_SELECTOR_WITNESS")
        .map(PathBuf::from)
        .ok_or("configure the private selector receipt")?;
    let source = Survey::load(&selection)?;
    let path = p4kx_testkit::real_archive_path().map_err(|_| "configure the installed Archive")?;
    let key = p4kx_testkit::real_archive_key().map_err(|_| "read the configured Archive key")?;
    let archive =
        Archive::open_with_key(path, Some(key)).map_err(|_| "open the installed Archive")?;
    let watch = ArchiveWatch::new(&archive);
    let build = archive
        .build()
        .ok_or("the installed Archive has no Build")?;
    if build.change_number() != source.build
        || [build.branch(), build.version(), build.build_id()]
            .into_iter()
            .any(str::is_empty)
    {
        return Err(
            "the private selection receipt does not match the installed Build change".into(),
        );
    }
    let (rows, unresolved, requested) = source.requests();
    if requested.is_empty() {
        return Err("the receipt has no reachable Loot candidates by physical origin".into());
    }
    let count: usize = requested.values().map(BTreeMap::len).sum();
    watch
        .check()
        .map_err(|_| "Archive changed before the source comparison")?;
    let result = with_entry_capture(&archive, |capture| inspect(&archive, &requested, capture))
        .map_err(|_| "cannot check the sealed selector source")?;
    watch
        .check()
        .map_err(|_| "Archive changed during the source comparison")?;
    let mut out = io::stdout().lock();
    writeln!(
        out,
        "loot_selector_shape change={} rows={rows} unresolved_routes={unresolved} owners={} members={count}",
        build.change_number(),
        requested.len()
    )?;
    for (state, count) in result {
        writeln!(
            out,
            "loot_selector_shape observation={state:?} count={count}"
        )?;
    }
    Ok(())
}

fn inspect(
    archive: &Archive,
    requested: &MemberRequests<'_>,
    capture: &EntryCapture<'_>,
) -> Result<BTreeMap<State, usize>, CatalogError> {
    let index = Index::new(archive);
    let basenames = root_basenames(&index);
    let mut counts = BTreeMap::new();
    for (&owner, members) in requested {
        let Some(outer) = archive
            .entries()
            .get(owner)
            .filter(|entry| is_socpak(entry.path()))
        else {
            *counts.entry(State::OwnerUnavailable).or_default() += expectation_count(members);
            continue;
        };
        let Ok(container) = archive.open_container(outer) else {
            *counts.entry(State::OwnerUnavailable).or_default() += expectation_count(members);
            continue;
        };
        let context = Owner {
            archive,
            outer,
            container: &container,
            index: &index,
            basenames: &basenames,
        };
        for (&member, expected) in members {
            let (state, matched) = read_member(&context, member, expected, capture)?;
            if matched {
                *counts.entry(state).or_default() += 1;
                *counts.entry(State::DifferentSelection).or_default() += expected.len() - 1;
            } else {
                *counts.entry(state).or_default() += expected.len();
            }
        }
    }
    counts.retain(|_, count| *count != 0);
    Ok(counts)
}

struct Owner<'source> {
    archive: &'source Archive,
    outer: &'source Entry,
    container: &'source Archive,
    index: &'source Index<'source>,
    basenames: &'source BTreeMap<String, usize>,
}

fn read_member(
    owner: &Owner<'_>,
    member: usize,
    expected: &BTreeSet<&str>,
    capture: &EntryCapture<'_>,
) -> Result<(State, bool), CatalogError> {
    let Some(inner) = owner
        .container
        .entries()
        .get(member)
        .filter(|entry| is_entxml(entry.path()))
    else {
        return Ok((State::MemberUnavailable, false));
    };
    let selected = EntrySource::inner(capture, owner.archive, owner.outer, owner.container, inner)?
        .operation(EntryOperation::EntityLinks);
    let Ok(bytes) = EntryRead::read_all(selected.receipt().clone(), owner.container, inner, None)?
    else {
        return Ok((State::MemberUnavailable, false));
    };
    let Ok(xml) = EntryXml::parse(&bytes, capture)? else {
        return Ok((State::ParseUnavailable, false));
    };
    if xml.is_lossy() {
        return Ok((State::ParseUnavailable, false));
    }
    let (_, Some(admitted)) = super::read_document(&xml, None)?.into_parts() else {
        return Ok((State::DifferentSelection, false));
    };
    if !expected.contains(admitted.as_str()) {
        return Ok((State::DifferentSelection, false));
    }
    let Some(shape) = selector_shape::inspect(owner.index, &xml, &admitted)? else {
        return Ok((State::UnboundSelection, true));
    };
    Ok((
        State::Observed(owner.observe(admitted.as_str(), shape)),
        true,
    ))
}

impl Owner<'_> {
    fn observe(&self, selector: &str, shape: Shape) -> Observation {
        let mut observation = Observation {
            shape,
            current_outer_matches: self.index.candidates(selector).len(),
            permitted_relative: false,
            exact_containing_matches: 0,
            owner_directory_leads: 0,
            root_basename_leads: 0,
        };
        let basename = selector.rsplit(['/', '\\']).next().unwrap_or_default();
        if !basename.is_empty() {
            let mut key = starmap_index::key(basename.trim());
            if !key.ends_with(".socpak") {
                key.push_str(".socpak");
            }
            observation.root_basename_leads = self.basenames.get(&key).copied().unwrap_or_default();
        }
        let Some(path) = ChildContainerPath::new(selector) else {
            return observation;
        };
        let relative = starmap_index::key(path.relative());
        observation.permitted_relative = true;
        observation.exact_containing_matches = self
            .container
            .entries()
            .iter()
            .filter(|entry| starmap_index::key(entry.path()) == relative)
            .count();
        if let Some((directory, _)) = self.outer.path().rsplit_once(['/', '\\']) {
            let authored = starmap_index::key(&format!("{directory}\\{}", path.relative()));
            if self.index.by_path.contains_key(&authored) {
                observation.owner_directory_leads = self.index.candidates(&authored).len();
            }
        }
        observation
    }
}

fn root_basenames(index: &Index<'_>) -> BTreeMap<String, usize> {
    let mut found = BTreeMap::new();
    for entry in index.entries.iter().filter(|entry| is_socpak(entry.path())) {
        let basename = entry.path().rsplit(['/', '\\']).next().unwrap_or_default();
        *found.entry(starmap_index::key(basename)).or_default() += 1;
    }
    found
}

fn expectation_count(members: &BTreeMap<usize, BTreeSet<&str>>) -> usize {
    members.values().map(BTreeSet::len).sum()
}

fn is_socpak(name: &str) -> bool {
    Path::new(name)
        .extension()
        .is_some_and(|extension| extension.eq_ignore_ascii_case("socpak"))
}

fn is_entxml(name: &str) -> bool {
    starmap_index::key(name)
        .split('\\')
        .any(|part| part == "entdata")
        && Path::new(name)
            .extension()
            .is_some_and(|extension| extension.eq_ignore_ascii_case("entxml"))
}

#[test]
fn receipt_reader_keeps_only_physical_loot_authors_with_basename_leads() -> TestResult {
    let input: Survey = serde_json::from_value(serde_json::json!({
        "build": "9",
        "sourceRows": [
            {"kind":"lootContainers","role":"derivedInput","selector":"synthetic/first.socpak",
             "authoringEntryRoutes":[[1,2],[1,2],[1,3,4]],
             "sameBasenameRootCandidates":1},
            {"kind":"lootContainers","role":"dependency","selector":"synthetic/second.socpak",
             "authoringEntryRoutes":[[1,3]],
             "sameBasenameRootCandidates":1},
            {"kind":"lootContainers","role":"derivedInput","selector":"synthetic/third.socpak",
             "authoringEntryRoutes":[[1,4]],
             "sameBasenameRootCandidates":0}
        ]
    }))?;
    let (rows, unresolved, physical) = input.requests();
    assert_eq!(
        (rows, unresolved),
        (1, 1),
        "irrelevant rows are excluded and deeper authors stay unknown"
    );
    assert_eq!(
        physical.values().map(BTreeMap::len).sum::<usize>(),
        1,
        "duplicate physical member receipts are grouped"
    );
    assert_eq!(
        physical[&1][&2].len(),
        1,
        "the exact selector expectation is retained once"
    );
    Ok(())
}
