// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::{Entities, LoadoutResolver};
use p4kx_extraction::SourceValueState;
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestPort, entity_schema};

use super::Capture;
use crate::build::original_input::ship_pool_test_input;
use crate::catalog_read::original::AuthoredEntryPath;
use crate::export_ledger::ExportLedger;
use crate::{CatalogError, ExportAccountingError, Kind};

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn database() -> Vec<u8> {
    entity_schema(DataForgeBuilder::new().version(8))
        .record(
            TestEntity::new(1, "Root", "synthetic/root.xml")
                .port(TestPort::new("shared"))
                .record(),
        )
        .build()
}

#[test]
fn a_name_cell_requires_the_same_original_value_and_input() -> TestResult {
    ship_pool_test_input::with_input(&database(), Vec::new(), |_, files, forge, input| {
        let capture = Arc::new(Capture::for_input(
            input,
            &ExportLedger::default(),
            None,
            Kind::Loadouts,
        ));
        let mut resolver = LoadoutResolver::with_observed_entities(
            Arc::new(Entities::new(forge)),
            capture.clone(),
        )
        .with_files(files);
        let record = forge.record(0).ok_or("actual original root")?;
        let node = resolver.resolve(&record)?;
        let [port] = node.ports() else {
            return Err("one genuine same-read port".into());
        };
        let proof = capture
            .port_name_proof(port.name(), port.name_proof())?
            .ok_or("actual accepted name getter")?;
        let cell = capture.port_name("shared", Some(&proof))?;
        assert_eq!(
            cell.state(),
            SourceValueState::Present,
            "actual original name"
        );
        assert!(
            matches!(
                capture.port_name("unrelated", Some(&proof)),
                Err(CatalogError::Accounting(
                    ExportAccountingError::ConflictingSourceDatum
                ))
            ),
            "equal owner alone cannot certify another copied value"
        );
        reject_foreign(&proof)?;
        Ok(())
    })
}

fn reject_foreign(proof: &AuthoredEntryPath) -> TestResult {
    ship_pool_test_input::with_input(&database(), Vec::new(), |_, _, _, input| {
        let foreign = Capture::for_input(input, &ExportLedger::default(), None, Kind::Loadouts);
        assert!(
            foreign.port_name("shared", Some(proof)).is_err(),
            "equal bytes and descriptors cannot supply another original input binding"
        );
        Ok(())
    })
}
