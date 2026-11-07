// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::{Arc, Mutex};

use p4kx_dataforge::DataForge;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestEntry};

use super::{Budget, Element, LoadoutDocumentKind};
use crate::{ArchiveFiles, Entities, LoadoutReadEvent, LoadoutReadObserver, LoadoutResolver};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Seen {
    Read(Option<usize>, bool),
    Parsed(Option<usize>, bool),
    Incomplete,
}

#[derive(Default)]
struct Observer(Mutex<Vec<Seen>>);

impl LoadoutReadObserver for Observer {
    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        let event = match event {
            LoadoutReadEvent::FileRead {
                source, outcome, ..
            } => Seen::Read(source.entry_index, outcome.is_ok()),
            LoadoutReadEvent::FileProjection {
                source, outcome, ..
            } => Seen::Parsed(source.entry_index, outcome.is_ok()),
            LoadoutReadEvent::InventoryIncomplete => Seen::Incomplete,
            _ => return ControlFlow::Continue(()),
        };
        let Ok(mut seen) = self.0.lock() else {
            panic!("file observer evidence mutex is poisoned");
        };
        seen.push(event);
        drop(seen);
        ControlFlow::Continue(())
    }
}

const CONFIGURED: &str = "Scripts/Entities/Vehicles/Implementations/Xml/patch.xml";

fn archive() -> Result<(tempfile::TempDir, p4kx_archive::Archive), Box<dyn std::error::Error>> {
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("Data.p4k");
    ArchiveBuilder::new()
        .entry(TestEntry::stored(
            format!("Data/{CONFIGURED}"),
            b"<Patch/>".to_vec(),
        ))
        .write_to(&path)?;
    Ok((directory, p4kx_archive::Archive::open(&path)?))
}

#[test]
fn one_patch_under_two_context_budgets_does_not_claim_conflicting_intrinsic_results()
-> Result<(), Box<dyn std::error::Error>> {
    let (_directory, archive) = archive()?;
    let files = ArchiveFiles::new(&archive);
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new()).build();
    let forge = DataForge::open(&database)?;
    let observer = Arc::new(Observer::default());
    let resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), observer.clone());
    for constrained in [true, false] {
        let bytes = crate::loadout_file_events::read(
            &files,
            CONFIGURED,
            LoadoutDocumentKind::VehiclePatch,
            resolver.observation.as_deref(),
        )?;
        let mut root = Element::read("Vehicle".to_owned(), Vec::new(), Vec::new());
        let mut budget = if constrained {
            Budget::without_elements()
        } else {
            Budget::new(bytes.len())
        };
        let result = super::patch(
            &mut root,
            &bytes,
            &mut budget,
            &files,
            CONFIGURED,
            resolver.observation.as_deref(),
        );
        assert_eq!(
            result.is_ok(),
            !constrained,
            "the actual context budget controls whether the projection can run"
        );
    }
    let Ok(seen) = observer.0.lock() else {
        return Err("file observer evidence mutex is poisoned".into());
    };
    assert_eq!(
        *seen,
        [
            Seen::Read(Some(0), true),
            Seen::Incomplete,
            Seen::Read(Some(0), true),
            Seen::Parsed(Some(0), true)
        ],
        "context exhaustion remains incomplete inventory instead of a false intrinsic file defect"
    );
    drop(seen);
    Ok(())
}
