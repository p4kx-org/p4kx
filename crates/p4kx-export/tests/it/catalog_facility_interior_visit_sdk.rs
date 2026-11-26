// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::FacilityInteriorPlacementSnapshot;
use p4kx_extraction::SourceValueState;
use serde::Serialize;
use serde_json::{Value, json};

use super::{TestResult, build, fixture, interior_rows, proof};
use crate::sdk_full_proof as full_proof;

#[test]
fn interior_visitor_preserves_physical_roots_catalog_order_and_sources() -> TestResult {
    let (_folder, archive) = fixture::archive(fixture::Case::VisitorRoots)?;
    let catalog = build(&archive)?;
    let session = full_proof::open(&archive, full_proof::NO_LANGUAGES, None)?;
    let mut seen = VisitedRooms::default();
    let summary = session
        .parsed(None)?
        .visit_facility_interior_placements(None, |snapshot| seen.retain(snapshot))?;
    assert!(matches!(
        summary.availability,
        DatasetAvailability::Available
    ));
    assert_eq!(
        seen.roots,
        [3, 4, 5, 0],
        "the physical Ship callbacks follow GUID order"
    );
    assert_eq!(summary.owner_count, 4);
    assert_eq!(summary.placement_count, 8);
    assert_eq!(summary.coverage_count, 5);
    assert_eq!(summary.incomplete_owner_count, seen.incomplete_owners);
    assert!(
        summary.incomplete_owner_count > 0,
        "missing rooms stay incomplete"
    );
    assert_eq!(
        seen.placements.in_catalog_order(),
        proof::with_origins(serde_json::to_value(interior_rows(&catalog)?)?, &catalog)?,
        "direct placements follow physical Record order and attached placements keep Loadout order"
    );
    assert_eq!(
        seen.coverage.in_catalog_order(),
        proof::with_origins(
            proof::at(&catalog, "/placements/facilityInteriorCoverage")?.clone(),
            &catalog
        )?,
        "room declarations, including missing containers, retain order and actual sources"
    );
    Ok(())
}

#[derive(Default)]
struct VisitedRooms {
    roots: Vec<u32>,
    incomplete_owners: usize,
    placements: RoomOrder,
    coverage: RoomOrder,
}

impl VisitedRooms {
    fn retain(&mut self, snapshot: &FacilityInteriorPlacementSnapshot) -> TestResult {
        let coverage = &snapshot.coverage;
        let ship = coverage.ship_record_index;
        assert_eq!(
            coverage.root_state,
            SourceValueState::Present,
            "the fixture Ship keeps its present physical root"
        );
        self.roots.push(ship);
        self.incomplete_owners += usize::from(!coverage.complete);
        match ship {
            4 => assert!(
                snapshot.rows.is_empty() && coverage.rows.is_empty(),
                "admitted empty Ship"
            ),
            5 => assert!(
                snapshot.rows.is_empty() && coverage.rows.len() == 1 && !coverage.complete,
                "admitted Ship with a missing original room"
            ),
            _ => {}
        }
        let sources = json!({ "provenance": coverage.source_inventory });
        for row in &snapshot.rows {
            let placement = row
                .interior_placement()
                .ok_or("a typed Interior placement")?;
            assert_eq!(
                row.ship_record_index().value(),
                Some(&ship),
                "the Interior placement retains its physical Ship Record"
            );
            self.placements
                .retain(ship, placement.loadout_position().is_some(), row, &sources)?;
        }
        for row in &coverage.rows {
            self.coverage
                .retain(ship, row.loadout_position().is_some(), row, &sources)?;
        }
        Ok(())
    }
}

#[derive(Default)]
struct RoomOrder {
    direct: BTreeMap<u32, Vec<Value>>,
    attached: Vec<Value>,
}

impl RoomOrder {
    fn retain(
        &mut self,
        ship: u32,
        attached: bool,
        row: &impl Serialize,
        sources: &Value,
    ) -> TestResult {
        let row = proof::with_origins(serde_json::to_value(row)?, sources)?;
        if attached {
            self.attached.push(row);
        } else {
            self.direct.entry(ship).or_default().push(row);
        }
        Ok(())
    }

    fn in_catalog_order(self) -> Value {
        Value::Array(
            self.direct
                .into_values()
                .flatten()
                .chain(self.attached)
                .collect(),
        )
    }
}
