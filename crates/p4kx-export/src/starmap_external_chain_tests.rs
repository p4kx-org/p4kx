// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::error::Error;

use p4kx_extraction::Archive;
use p4kx_testkit::{TestContainer, TestElement, TestEntry};

use super::{Limits, child, external, external_at, guid, open, read, root, socpak, system};
use crate::catalog_read::original::with_entry_capture;
use crate::starmap_index::Index;
use crate::starmap_parse::read_all;
use crate::starmap_walk::{System, SystemRead};
use crate::{ExportDiagnosticCode, ExportDiagnosticStage, Kind};

use crate::starmap_containers::Placements;

type TestResult<T = ()> = Result<T, Box<dyn Error>>;

#[test]
fn external_container_routes_reach_original_placements_beyond_32_edges() -> TestResult {
    const EDGES: u16 = 40;

    let dir = tempfile::tempdir()?;
    let mut entries = vec![socpak(
        "tsystem",
        &root(vec![external_at("hop-0", "1,0,0")]),
    )];
    for step in 0..EDGES {
        let child = if step + 1 == EDGES {
            child("2,0,0", 1)
        } else {
            external_at(&format!("hop-{}", step + 1), "1,0,0")
        };
        entries.push(socpak(&format!("hop-{step}"), &root(vec![child])));
    }
    let placements = read(dir.path(), entries, Limits::default());
    let assigned = placements
        .get(guid(1))
        .ok_or("the original terminal Record is placed")?;
    assert_eq!(
        assigned.position,
        [f64::from(EDGES + 2), 0.0, 0.0],
        "each original external transform contributes to the terminal position",
    );
    assert_eq!(
        assigned.proof.last.steps().count(),
        usize::from(EDGES + 1),
        "every actual external Child and the terminal Child remains in the route",
    );
    assert_eq!(
        placements.report.deepest_external(),
        usize::from(EDGES),
        "the deepest reached external route is reported",
    );
    assert!(
        placements
            .ledger
            .entries
            .iter()
            .all(|entry| !entry.disposition.failed()),
        "the reached physical containers produce no unattempted read failures",
    );
    assert_eq!(placements.counts.malformed, 0, "no route is clipped");
    Ok(())
}

#[test]
fn a_shared_container_keeps_each_route_and_only_its_active_cycle_is_skipped() -> TestResult {
    let dir = tempfile::tempdir()?;
    let first = external_at("shared", "10,0,0")
        .child(TestElement::new("ChildObjectContainers").child(child("2,0,0", 2)));
    let entries = system(
        &root(vec![first, external_at("shared", "20,0,0")]),
        &[(
            "shared",
            root(vec![
                child("1,0,0", 1),
                external("shared"),
                child("3,0,0", 3),
            ]),
        )],
    );
    let archive = open(dir.path(), entries);
    let result = walk(&archive)?;

    let placed: Vec<_> = result
        .placed
        .iter()
        .map(|(id, row)| (*id, row.position))
        .collect();
    assert_eq!(
        placed,
        [
            (guid(1), [11.0, 0.0, 0.0]),
            (guid(3), [13.0, 0.0, 0.0]),
            (guid(2), [12.0, 0.0, 0.0]),
            (guid(1), [21.0, 0.0, 0.0]),
            (guid(3), [23.0, 0.0, 0.0]),
        ],
        "external descendants are composed before inline descendants and the next sibling",
    );
    assert_eq!(
        result.malformed, 2,
        "the active physical cycle is skipped for each route"
    );
    assert_eq!(
        result.unreadable, 0,
        "a repeated occurrence remains readable"
    );
    let root_slots = result
        .placed
        .iter()
        .filter(|(id, _)| *id == guid(1))
        .map(|(_, row)| {
            row.proof
                .last
                .steps()
                .last()
                .map(crate::starmap_containers::proof::PlacementStep::child_index)
        })
        .collect::<Vec<_>>();
    let [Some(first), Some(second)] = root_slots.as_slice() else {
        return Err("expected both original root slots".into());
    };
    assert_ne!(
        first, second,
        "the same input container keeps distinct authored ancestry"
    );
    Ok(())
}

#[test]
fn duplicate_external_targets_keep_inline_children_and_refuse_a_payload_winner() -> TestResult {
    let directory = tempfile::tempdir()?;
    let external = external_at("shared", "10,0,0")
        .child(TestElement::new("ChildObjectContainers").child(child("2,0,0", 3)));
    let entries = system(
        &root(vec![external]),
        &[
            ("shared", root(vec![child("1,0,0", 1)])),
            ("shared", root(vec![child("7,0,0", 2)])),
        ],
    );
    let placements = read(directory.path(), entries, Limits::default());
    assert!(
        placements.get(guid(1)).is_none(),
        "first duplicate was not placed"
    );
    assert!(
        placements.get(guid(2)).is_none(),
        "second duplicate was not placed"
    );
    assert_eq!(
        placements.get(guid(3)).map(|row| row.position),
        Some([12.0, 0.0, 0.0]),
        "inline children still follow their original parent",
    );
    assert_eq!(placements.counts.missing, 0, "a duplicate is not missing");
    assert_eq!(
        candidates(placements)?,
        [(Some(1), None), (Some(2), None)],
        "both original outer directory positions remain in the selection ledger",
    );
    Ok(())
}

#[test]
fn duplicate_named_root_is_not_opened_as_a_detached_candidate() -> TestResult {
    let directory = tempfile::tempdir()?;
    let placements = read(
        directory.path(),
        vec![
            socpak("tsystem", &root(vec![child("1,0,0", 1)])),
            socpak("tsystem", &root(vec![child("7,0,0", 2)])),
        ],
        Limits::default(),
    );
    assert_eq!(
        placements.counts.systems, 1,
        "one system folder was discovered"
    );
    assert!(placements.get(guid(1)).is_none() && placements.get(guid(2)).is_none());
    assert!(
        placements.opened.is_empty(),
        "a duplicate root cannot seed a detached walk"
    );
    assert_eq!(candidates(placements)?, [(Some(0), None), (Some(1), None)]);
    Ok(())
}

#[test]
fn duplicate_inner_descriptors_have_exact_directory_candidates() -> TestResult {
    for descriptor in ["tsystem.xml", "fallback.xml"] {
        let directory = tempfile::tempdir()?;
        let bytes = TestContainer::new()
            .entry(TestEntry::stored(descriptor, root(vec![child("1,0,0", 1)])))
            .entry(TestEntry::stored(descriptor, root(vec![child("7,0,0", 2)])))
            .build();
        let placements = read(
            directory.path(),
            vec![TestEntry::stored(
                format!("{}tsystem.socpak", super::FOLDER),
                bytes,
            )],
            Limits::default(),
        );
        assert!(placements.get(guid(1)).is_none() && placements.get(guid(2)).is_none());
        assert_eq!(
            candidates(placements)?,
            [(Some(0), Some(0)), (Some(0), Some(1))]
        );
    }
    Ok(())
}

fn candidates(placements: Placements) -> TestResult<Vec<(Option<usize>, Option<usize>)>> {
    let compiled = crate::export_health_entries::compile(placements.ledger.entries)?;
    Ok(compiled
        .failed
        .iter()
        .filter(|entry| {
            entry.source.kind == Kind::StarmapObjects
                && entry.code == ExportDiagnosticCode::AmbiguousIdentity
                && entry.stage == ExportDiagnosticStage::Selection
        })
        .map(|entry| (entry.source.entry_index, entry.source.container_entry_index))
        .collect())
}

fn walk(archive: &Archive) -> TestResult<SystemRead> {
    let index = Index::new(archive);
    let systems = index.systems();
    let (name, root) = systems
        .first()
        .ok_or("the real synthetic system root is admitted")?;
    let limits = Limits::default();
    Ok(with_entry_capture(archive, |capture| {
        let sources = read_all(
            archive,
            &index,
            &[*root],
            None,
            limits.source_children,
            capture,
        )?;
        assert_eq!(sources.parsed, 2, "each physical container is parsed once");
        System::run(
            &index,
            &sources.bodies,
            name,
            *root,
            None,
            limits,
            (archive, capture, None),
        )
    })?)
}
