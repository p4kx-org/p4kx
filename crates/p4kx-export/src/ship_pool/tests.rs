// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{ArchiveFiles, Entities};

use super::ShipPool;

use p4kx_extraction::Archive;
use p4kx_testkit::{ArchiveBuilder, DataForgeBuilder, TestRecord, TestStruct};

#[test]
fn a_poisoned_observed_pool_cannot_supply_a_ship_reader() -> Result<(), Box<dyn std::error::Error>>
{
    let directory = tempfile::tempdir()?;
    let path = directory.path().join("synthetic.p4k");
    std::fs::write(&path, ArchiveBuilder::new().build())?;
    let archive = Archive::open(&path)?;
    let files = ArchiveFiles::new(&archive);
    let bytes = DataForgeBuilder::new()
        .structure(TestStruct::new("Source"))
        .record(TestRecord::new(
            1,
            "Source",
            "Source.One",
            "synthetic/source.xml",
        ))
        .build();
    crate::build::original_input::test_support::with_input(&bytes, |forge, input| {
        let record = forge.record(0).ok_or("source")?;
        let tree = record.tree()?;
        let pool = ShipPool::new(Arc::new(Entities::new(forge)), &files);
        assert!(
            crate::export_evidence_lock::poison(&pool.observed),
            "the fixture poisons the actual observed reader pool"
        );
        let result = pool.read_catalog(
            &record,
            &tree,
            &crate::export_ledger::ExportLedger::default(),
            input,
            None,
        );
        assert!(
            matches!(
                result,
                Err(crate::CatalogError::Accounting(
                    crate::ExportAccountingError::PoisonedEvidence
                ))
            ),
            "no cached reader or evidence is recovered from a poisoned pool"
        );
        Ok(())
    })
}
