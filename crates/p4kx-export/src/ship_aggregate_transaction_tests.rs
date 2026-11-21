// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use p4kx_assembly::{
    Entities, LoadoutPath, LoadoutPathObserver, LoadoutReadEvent, LoadoutReadObserver,
    LoadoutRequest, LoadoutSelection, VehicleImplementation, XmlLoadoutPathSite,
};
use p4kx_extraction::{CancelToken, CanonicalReadObserver, DataForgeError, Record, RecordTree};
use p4kx_testkit::{DataForgeBuilder, TestEntity, TestEntry};

use super::{CatalogRead, ObservedReader, ShipPool};
use crate::build::original_input::OriginalCatalogInput;
use crate::build::original_input::ship_pool_test_input;
use crate::build::original_input::test_support::TestResult;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::loadout::ledger::Capture;
use crate::record_observation::RecordObservation;
use crate::ship_base::ShipReader;
use crate::{CatalogError, ExportAccountingError, Kind};

#[derive(Clone, Copy)]
enum Fault {
    Cancel,
    Observer,
}

#[test]
fn cancellation_after_real_vehicle_capture_returns_no_aggregate_or_cached_reader() -> TestResult {
    run(Fault::Cancel)
}

#[test]
fn fatal_observer_after_real_vehicle_capture_returns_no_aggregate_or_cached_reader() -> TestResult {
    run(Fault::Observer)
}

fn run(fault: Fault) -> TestResult {
    let database = p4kx_testkit::entity_schema(DataForgeBuilder::new())
        .record(
            TestEntity::new(
                0x7521,
                "TransactionShip",
                "libs/foundry/records/entities/spaceships/transaction.xml",
            )
            .vehicle(
                "Scripts/Entities/Vehicles/Implementations/Xml/TransactionShip.xml",
                "",
            )
            .record(),
        )
        .build();
    let entries = vec![TestEntry::stored(
        "Data/Scripts/Entities/Vehicles/Implementations/Xml/TransactionShip.xml",
        b"<Vehicle><Parts><Part mass=\"12\" damageMax=\"8\"/></Parts></Vehicle>".as_slice(),
    )];
    ship_pool_test_input::with_input(&database, entries, |_, files, forge, input| {
        let record = forge.record(0).ok_or("actual original source")?;
        let tree = record.tree()?;
        let source = crate::export_field_walk::inspect(
            &tree,
            &record,
            Some(Kind::Ships),
            ExportSourceRole::Selected,
            None,
        )?;
        let operands = crate::catalog_v3::authored_fields::ship_source_read::aggregate_for_test(
            &tree,
            input,
            &RecordObservation::capture(input, &record)?,
        )?;
        let pool = ShipPool::new(Arc::new(Entities::new(forge)), files);
        let cancel = CancelToken::new();
        let gate = inject_fault(&pool, input, &source, &cancel, fault)?;
        let result = pool.read_catalog(&operands, &record, &tree, &source, input, Some(&cancel));
        check_rejection(&pool, &gate, &result)?;
        let retry = pool.read_catalog(&operands, &record, &tree, &source, input, None)?;
        let aggregates = retry.aggregates.ok_or("fresh complete retry result")?;
        assert_eq!(aggregates.mass.value(), Some(&12.0), "retry mass");
        assert_eq!(aggregates.hull_health.value(), Some(&8.0), "retry health");
        assert!(
            aggregates.mass.origin().source_set_complete(),
            "retry origin"
        );
        assert_eq!(
            pool.observed
                .lock()
                .map_err(|_| "actual successful pool")?
                .len(),
            1,
            "only a completed reader returns to the pool"
        );
        Ok(())
    })
}

fn inject_fault<'input>(
    pool: &ShipPool<'input>,
    input: &'input OriginalCatalogInput<'input, 'input>,
    source: &ExportLedger,
    cancel: &CancelToken,
    fault: Fault,
) -> TestResult<Arc<Gate<'input>>> {
    let capture = Arc::new(Capture::for_input(input, source, Some(cancel), Kind::Ships));
    let gate = Arc::new(Gate {
        capture: Arc::clone(&capture),
        cancel: cancel.clone(),
        fault,
        calls: AtomicUsize::new(0),
    });
    pool.observed
        .lock()
        .map_err(|_| "actual initial pool")?
        .push(ObservedReader {
            reader: ShipReader::with_observed_entities(
                Arc::clone(&pool.entities),
                pool.files,
                Arc::clone(&gate),
            ),
            capture,
        });
    Ok(gate)
}

fn check_rejection(
    pool: &ShipPool<'_>,
    gate: &Gate<'_>,
    result: &Result<CatalogRead<'_>, CatalogError>,
) -> TestResult {
    assert_eq!(
        gate.calls.load(Ordering::SeqCst),
        1,
        "fault follows actual current-caller capture"
    );
    match gate.fault {
        Fault::Cancel => assert!(
            matches!(result, Err(CatalogError::Cancelled)),
            "outer read propagates cancellation"
        ),
        Fault::Observer => assert!(
            matches!(
                result,
                Err(CatalogError::Accounting(
                    ExportAccountingError::MissingFieldInspection
                ))
            ),
            "outer read propagates the observer failure"
        ),
    }
    assert!(
        pool.observed
            .lock()
            .map_err(|_| "actual failed pool")?
            .is_empty(),
        "a failed observer cannot return a staged reader"
    );
    Ok(())
}

struct Gate<'input> {
    capture: Arc<Capture<'input>>,
    cancel: CancelToken,
    fault: Fault,
    calls: AtomicUsize,
}

impl LoadoutReadObserver for Gate<'_> {
    fn observes_vehicle_totals(&self) -> bool {
        self.capture.observes_vehicle_totals()
    }

    fn vehicle_used(
        &self,
        record_index: u32,
        vehicle: &VehicleImplementation,
        caller: &LoadoutRequest,
    ) -> ControlFlow<()> {
        self.capture.vehicle_used(record_index, vehicle, caller)?;
        self.calls.fetch_add(1, Ordering::SeqCst);
        match self.fault {
            Fault::Cancel => self.cancel.cancel(),
            Fault::Observer => self.capture.missing_root(),
        }
        ControlFlow::Break(())
    }

    fn observe(&self, event: LoadoutReadEvent<'_, '_>) -> ControlFlow<()> {
        LoadoutReadObserver::observe(self.capture.as_ref(), event)
    }

    fn with_record_paths(
        &self,
        record: &Record<'_>,
        tree: &RecordTree<'_>,
        parse: &mut dyn FnMut(Option<&dyn LoadoutPathObserver>) -> ControlFlow<()>,
    ) -> ControlFlow<()> {
        self.capture.with_record_paths(record, tree, parse)
    }

    fn xml_path(
        &self,
        selection: &LoadoutSelection,
        site: &XmlLoadoutPathSite<'_>,
    ) -> ControlFlow<()> {
        self.capture.xml_path(selection, site)
    }

    fn derived_path(
        &self,
        selection: &LoadoutSelection,
        inputs: &[&LoadoutPath],
        value: &str,
    ) -> ControlFlow<()> {
        self.capture.derived_path(selection, inputs, value)
    }
}

impl CanonicalReadObserver for Gate<'_> {
    fn observe(
        &self,
        record: &Record<'_>,
        outcome: Result<&RecordTree<'_>, &DataForgeError>,
    ) -> ControlFlow<()> {
        CanonicalReadObserver::observe(self.capture.as_ref(), record, outcome)
    }
}
