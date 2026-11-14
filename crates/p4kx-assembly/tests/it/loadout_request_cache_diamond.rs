// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::{Arc, Mutex};

use p4kx_assembly::{Entities, LoadoutFileError, LoadoutFiles, LoadoutResolver};
use p4kx_dataforge::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout};

use crate::loadout_request_authorship_support as request_source;
use crate::loadout_request_authorship_support::{EventKind, Probe, State, TestResult};

const ROOT: &str = "synthetic/root-loadout.xml";
const B: &str = "synthetic/b.xml";
const C: &str = "synthetic/c.xml";
const D: &str = "synthetic/d.xml";

#[derive(Default)]
struct Files(Mutex<[usize; 4]>);

impl LoadoutFiles for Files {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        let (index, bytes): (usize, &[u8]) = match path {
            ROOT => (0, br#"<Loadout><Loadouts><Loadout loadout="synthetic/b.xml"/><Loadout loadout="synthetic/c.xml"/><Loadout loadout="synthetic/d.xml"/></Loadouts></Loadout>"#),
            B => (1, b"<Loadout/>"),
            C => (2, br#"<Loadout><Loadouts><Loadout loadout="synthetic/b.xml"/></Loadouts></Loadout>"#),
            D => (3, br#"<Loadout><Loadouts><Loadout loadout="synthetic/c.xml"/></Loadouts></Loadout>"#),
            _ => return Err(LoadoutFileError::NotFound),
        };
        let mut reads = self.0.lock().map_err(|_| LoadoutFileError::Unreadable {
            reason: "the synthetic read counter is poisoned".to_owned(),
        })?;
        let count = reads
            .get_mut(index)
            .ok_or_else(|| LoadoutFileError::Unreadable {
                reason: "the synthetic path has no read-counter slot".to_owned(),
            })?;
        *count += 1;
        drop(reads);
        Ok(bytes.to_vec())
    }
}

#[test]
fn locally_reused_include_retains_dependencies_loaded_before_its_own_read() -> TestResult {
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Root", "synthetic/entity.xml")
                .loadout(TestLoadout::Xml(ROOT.to_owned()))
                .record(),
        )
        .build();
    let forge = DataForge::open(&database)?;
    let files = Files::default();
    let probe = Arc::new(Probe::default());
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), probe.clone())
            .with_files(&files);
    let entity = forge.record(0).ok_or("the synthetic Entity is absent")?;
    resolver.resolve(&entity)?;
    resolver.resolve(&entity)?;
    let counts = files
        .0
        .lock()
        .map_err(|_| "the synthetic read counter is poisoned")?;
    assert_eq!(
        *counts, [1; 4],
        "dependency receipts must not cause another provider read"
    );
    drop(counts);
    let state = probe
        .0
        .lock()
        .map_err(|_| "the request evidence mutex is poisoned")?;
    verify_closure(&state)?;
    verify_shared_replay(&state)
}

fn verify_closure(state: &State) -> TestResult {
    let edges: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Dependency)
        .collect();
    let expected = [(ROOT, B), (C, B), (ROOT, C), (D, C), (ROOT, D)];
    assert_eq!(
        edges.len(),
        expected.len(),
        "every actual direct Include is observed once"
    );
    for (edge, (parent_path, child_path)) in edges.into_iter().zip(expected) {
        let parent = original(state, parent_path).ok_or("the real parent read is absent")?;
        let child = original(state, child_path).ok_or("the real child read is absent")?;
        assert!(
            edge.parent
                .as_ref()
                .is_some_and(|source| source.is_same_operation(&parent.observation)),
            "the dependency retains its actual including operation"
        );
        assert!(
            edge.observation.is_same_operation(&child.observation),
            "a dependency references its original child operation without another physical read"
        );
        assert_eq!(
            edge.request.selection(),
            child.request.selection(),
            "the first-population candidate remains separate from the Include edge"
        );
        let dependency = edge
            .dependency
            .as_ref()
            .and_then(request_source::caller_path)
            .ok_or("the actual direct Include request is absent")?;
        let site = state
            .xml
            .iter()
            .find(|site| {
                site.value == child_path && site.observation.is_same_operation(&parent.observation)
            })
            .ok_or("the direct Include's real same-parse site is absent")?;
        assert_eq!(
            dependency.selection(),
            Some(&site.selection),
            "C-to-B and D-to-C retain their own genuine Include selections"
        );
        assert!(
            edge.caller.is_none(),
            "an original dependency edge does not invent today's caller"
        );
    }
    Ok(())
}

fn verify_shared_replay(state: &State) -> TestResult {
    let replayed: Vec<_> = state
        .events
        .iter()
        .filter(|event| event.kind == EventKind::Reused)
        .collect();
    assert_eq!(
        replayed.len(),
        1,
        "global XML reuse links only its existing root operation"
    );
    let event = replayed
        .first()
        .ok_or("the actual global XML reuse is absent")?;
    let root = original(state, ROOT).ok_or("the original root read is absent")?;
    assert!(
        event.observation.is_same_operation(&root.observation),
        "the root link must retain the exact original physical operation"
    );
    assert_eq!(
        event.request.selection(),
        root.request.selection(),
        "the root link must retain the original candidate"
    );
    request_source::assert_current_caller(state, event, 0)?;
    assert!(
        event.dependency.is_none(),
        "global reuse does not relabel an original Include edge"
    );
    Ok(())
}

fn original<'state>(state: &'state State, path: &str) -> Option<&'state request_source::Event> {
    state
        .events
        .iter()
        .find(|event| event.kind == EventKind::Read && event.request.value() == path)
}
