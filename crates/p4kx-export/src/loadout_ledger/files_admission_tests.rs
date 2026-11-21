// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{
    Entities, LoadoutError, LoadoutFileError, LoadoutFiles, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutResolver,
};
use p4kx_extraction::DataForge;
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestLoadout};

use super::super::Capture;
use super::CatalogError;
use crate::export_ledger::ExportLedger;

struct Unavailable;

impl LoadoutFiles for Unavailable {
    fn read(&self, path: &str) -> Result<Vec<u8>, LoadoutFileError> {
        assert_eq!(
            path, "synthetic/admission.xml",
            "the actual requested source"
        );
        Err(LoadoutFileError::SourceEvidenceUnavailable)
    }
}

fn database() -> Vec<u8> {
    p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(1, "Admission", "synthetic/admission-record.xml")
                .loadout(TestLoadout::Xml("synthetic/admission.xml".to_owned()))
                .record(),
        )
        .build()
}
#[test]
fn reported_unavailable_loadout_admission_remains_the_first_fatal_adapter_cause()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = database();
    let forge = DataForge::open(&bytes)?;
    let record = forge.record(0).ok_or("actual source Record")?;
    assert_eq!(
        record.tree()?.error_count(),
        0,
        "a real readable source names the configured file"
    );
    let capture = Arc::new(Capture::new(&ExportLedger::default(), None));
    let mut resolver =
        LoadoutResolver::with_observed_entities(Arc::new(Entities::new(&forge)), capture.clone())
            .with_files(&Unavailable);
    assert!(
        matches!(
            resolver.resolve(&record),
            Err(LoadoutError::ObservationStopped)
        ),
        "the genuine original file event stops the actual adapter immediately"
    );
    assert!(
        capture
            .observe(LoadoutReadEvent::InventoryIncomplete)
            .is_break(),
        "a subsequent event cannot resume the stopped capture"
    );
    let outcome = capture.finish();
    assert!(
        matches!(
            outcome,
            Err(CatalogError::LoadoutSourceEvidence(
                LoadoutFileError::SourceEvidenceUnavailable
            ))
        ),
        "the first typed admission failure is neither cancellation nor an ordinary Health row"
    );
    Ok(())
}
