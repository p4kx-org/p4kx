// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::time::{Instant, UNIX_EPOCH};

use p4kx_export::catalog_queries::equipment::{
    InstalledConfigurationSnapshot, InstalledConfigurationVisitSummary,
};
use p4kx_export::catalog_queries::{
    CatalogLanguageSelection, CatalogQuerySession, DatasetAvailability,
};
use p4kx_extraction::{Archive, SourceValueState};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};

use super::TABLE;
use crate::real_archive_catalog_table_parity::{
    StreamedFlightDigests, TestResult, with_bound_measurement_from_producer,
};
use crate::real_archive_marker_sdk_ab::{ProbeMode, Rows, Workload, finish_lower, write_receipt};
use crate::real_archive_scoped_equipment_sdk::equipment_input::{
    full_source_session, with_archive,
};
use crate::real_archive_verification::print_line;

const PRODUCER: &str = "P4KX_FLIGHT_CATALOG_PRODUCER_REVISION";
const RECEIPT: &str = "P4KX_FLIGHT_FLEET_MARKER_RECEIPT";

#[test]
#[ignore = "installed Archive: configure the exact Build and a new private Marker receipt"]
fn installed_flight_fleet_cold_marker_visit() -> TestResult {
    let path = receipt_path()?;
    let revision = requested_revision()?;
    let clock = Instant::now();
    let mut receipt = None;
    with_archive(|archive| {
        let session =
            CatalogQuerySession::open(archive, CatalogLanguageSelection::Selected(&[]), None)?;
        let flight = read_fleet(archive, &session, ProbeMode::MarkersOnly, &revision, |_| {
            Ok(())
        })?;
        flight.report("Marker cold visit and verification", &clock)?;
        receipt = Some(json!({
            "witnessLane": "default-marker",
            "comparison": flight.comparison,
        }));
        Ok(())
    })?;
    write_receipt(
        &path,
        &receipt.ok_or("missing the terminal Flight Marker observation")?,
    )
}

#[test]
#[ignore = "same Build: configure the sealed Marker receipt and exact completed Normal producer"]
fn installed_flight_fleet_matches_retained_complete_catalog() -> TestResult {
    let marker: Value = serde_json::from_reader(File::open(receipt_path()?)?)?;
    if marker.get("witnessLane").and_then(Value::as_str) != Some("default-marker") {
        return Err("the Fleet comparison needs the independent default Marker receipt".into());
    }
    let revision = requested_revision()?;
    with_bound_measurement_from_producer(&revision, |archive, measurement| {
        let scratch = tempfile::tempdir()?;
        let mut comparison = StreamedFlightDigests::new(scratch.path());
        let clock = Instant::now();
        let flight = {
            let session = full_source_session(archive, CatalogLanguageSelection::Selected(&[]))?;
            read_fleet(archive, &session, ProbeMode::Full, &revision, |owner| {
                comparison.retain(owner)
            })?
        };
        if marker.get("comparison") != Some(&flight.comparison) {
            return Err(
                "Flight Marker and Full Build, census, values, or encryption markers differ".into(),
            );
        }
        flight.report("Full visit and verification", &clock)?;
        let counts = comparison.compare(measurement, TABLE, flight.summary.availability)?;
        if counts.rows != [flight.summary.placement_count] {
            return Err("the retained Flight table has a different terminal row count".into());
        }
        print_line(&format!(
            "Complete Flight fleet parity: rows={} originalSourceUses={} physicalSources={}",
            counts.rows[0], counts.source_uses, counts.physical_sources,
        ));
        Ok(())
    })
}

struct FleetEvidence {
    summary: InstalledConfigurationVisitSummary,
    comparison: Value,
}

impl FleetEvidence {
    fn report(&self, lane: &str, clock: &Instant) -> TestResult {
        let elapsed = clock.elapsed().as_millis();
        let terminal = self
            .comparison
            .get("terminal")
            .ok_or("missing Flight terminal census")?;
        print_line(&format!(
            "Flight {lane}: elapsedMillis={elapsed} {terminal}"
        ));
        Ok(())
    }
}

fn read_fleet(
    archive: &Archive,
    session: &CatalogQuerySession<'_>,
    mode: ProbeMode,
    revision: &str,
    mut retain: impl FnMut(&InstalledConfigurationSnapshot) -> TestResult,
) -> TestResult<FleetEvidence> {
    if session.provenance_mode() != mode.sdk() {
        return Err("the session uses a different Flight provenance mode".into());
    }
    let build = archive
        .build()
        .ok_or("the installed Archive has no Build")?;
    let mut census = FleetCensus::default();
    let mut rows = Rows::new(mode, Workload::Verification)?;
    let summary = session.visit_installed_configurations(None, |owner| -> TestResult {
        if owner.build.as_ref() != Some(build) {
            return Err("a Flight owner has a different Build from the Archive".into());
        }
        census.observe(owner)?;
        if matches!(mode, ProbeMode::MarkersOnly) || !owner.rows.is_empty() {
            rows.observe_inventory(&owner.source_inventory)?;
        }
        for row in &owner.rows {
            rows.observe_row(row)?;
        }
        retain(owner)
    })?;
    if summary.build.as_ref() != Some(build) {
        return Err("the terminal Flight summary has a different Build from the Archive".into());
    }
    let terminal = census.finish(&summary)?;
    let evidence = rows.finish(&terminal)?;
    Ok(FleetEvidence {
        summary,
        comparison: json!({
            "protocol": "p4kx-flight-fleet-v1",
            "producerRevision": revision,
            "buildIdentity": archive_identity(archive)?,
            "terminal": terminal,
            "valueMarkerCoverageDigests": evidence.value_marker_digests(),
        }),
    })
}

fn archive_identity(archive: &Archive) -> TestResult<Value> {
    let build = archive
        .build()
        .ok_or("the installed Archive has no Build")?;
    let modified = archive
        .file_modified()
        .ok_or("the installed Archive has no modification stamp")?
        .duration_since(UNIX_EPOCH)?;
    Ok(json!({
        "branch": build.branch(),
        "version": build.version(),
        "changeNumber": build.change_number(),
        "buildId": build.build_id(),
        "archiveBytes": archive.file_len(),
        "archiveModified": modified,
    }))
}

#[derive(Default)]
struct FleetCensus {
    owners: HashSet<u32>,
    owner_digest: Sha256,
    rows: usize,
    present_empty: usize,
    absent: usize,
    unreadable: usize,
    incomplete: usize,
}

impl FleetCensus {
    fn observe(&mut self, owner: &InstalledConfigurationSnapshot) -> TestResult {
        let index = owner.ship_record_index;
        if !self.owners.insert(index) {
            return Err("the Flight visitor repeated a physical Ship Record".into());
        }
        let mut positions = HashSet::new();
        for row in &owner.rows {
            let position = row
                .position()
                .value()
                .ok_or("a Flight row has no original position")?;
            if row.ship_record_index().value() != Some(&index)
                || position.loadout_record_index() != index
                || !positions.insert(position.port_ordinals())
            {
                return Err("a Flight row has a foreign or duplicate physical occurrence".into());
            }
        }
        let disposition = match (owner.loadout_state, owner.rows.is_empty()) {
            (SourceValueState::Present, false) => "present",
            (SourceValueState::Present, true) => {
                self.present_empty += 1;
                "present-empty"
            }
            (SourceValueState::Absent, true) => {
                self.absent += 1;
                "absent"
            }
            (SourceValueState::Unreadable, true) => {
                self.unreadable += 1;
                "unreadable"
            }
            _ => return Err("a Flight owner has an invalid Loadout disposition".into()),
        };
        let complete = owner.is_complete();
        self.incomplete += usize::from(!complete);
        self.rows += owner.rows.len();
        self.owner_digest.update(index.to_be_bytes());
        self.owner_digest.update([u8::from(complete)]);
        self.owner_digest.update(disposition.as_bytes());
        self.owner_digest.update([0]);
        Ok(())
    }

    fn finish(self, summary: &InstalledConfigurationVisitSummary) -> TestResult<Value> {
        let coverage = &summary.ship_coverage;
        if !coverage.unreadable_headers.is_empty()
            || !coverage.unreadable_bodies.is_empty()
            || !coverage.incomplete_fields.is_empty()
        {
            return Err("Flight Ship admission has unreadable original records".into());
        }
        if self.owners.len() != summary.owner_count
            || self.incomplete != summary.incomplete_owner_count
            || self.rows != summary.placement_count
        {
            return Err(
                "Flight terminal counts differ from observed physical Ships or rows".into(),
            );
        }
        require_available(summary)?;
        Ok(json!({
            "availability": summary.availability,
            "owners": summary.owner_count,
            "presentEmptyOwners": self.present_empty,
            "absentOwners": self.absent,
            "unreadableOwners": self.unreadable,
            "incompleteOwners": self.incomplete,
            "skippedLeftovers": coverage.skipped_leftovers,
            "placements": summary.placement_count,
            "ownerCensusSha256": finish_lower(self.owner_digest)?,
        }))
    }
}

const fn require_available(
    summary: &InstalledConfigurationVisitSummary,
) -> Result<(), &'static str> {
    match summary.availability {
        DatasetAvailability::Absent => Err("the installed Build lacks Flight source declarations"),
        DatasetAvailability::Unavailable => {
            Err("the installed Build's Flight source declarations are unreadable")
        }
        DatasetAvailability::Available if summary.owner_count == 0 => {
            Err("Flight sources exist but the Build has no admitted Ship owners")
        }
        DatasetAvailability::Available if summary.placement_count == 0 => {
            Err("Flight sources and Ship owners exist but no installed occurrences were found")
        }
        DatasetAvailability::Available => Ok(()),
        _ => Err("unrecognized Flight source availability"),
    }
}

fn receipt_path() -> TestResult<PathBuf> {
    Ok(PathBuf::from(
        env::var_os(RECEIPT).ok_or("configure a private Flight Marker receipt")?,
    ))
}

fn requested_revision() -> TestResult<String> {
    let revision = env::var(PRODUCER)?;
    if revision.len() != 40 || !revision.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err("configure the exact 40-digit Flight producer revision".into());
    }
    Ok(revision)
}
