// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use p4kx_export::catalog_queries::DatasetAvailability;
use p4kx_export::catalog_queries::equipment::FacilityInteriorPlacementSnapshot;
use p4kx_extraction::SourceValueState;
use serde::Serialize;
use serde_json::Value;

use super::super::compare_rows::Projection;
use super::super::digest_parity::{capture_inventory, capture_physical_sources, fingerprint};
use super::super::{BoundMeasurement, Counts, TestResult, collect_references, wire_row};
use super::digest_runs::Fingerprint;
use super::interior_loadouts::{LOADOUTS, ShipLoadoutDigest};
use super::placement::collect_input_refs;
use super::wire::{EncounterRow, OrderedRows};
use super::{StreamedComponentDigests, StreamedRowPaths, spill};

const AVAILABILITY: &str = "/placements/facilityInstances/availability";
const PATHS: [&str; 2] = [
    "/placements/facilityInteriorCoverage",
    "/placements/facilityInstances/rows",
];
const SHAPES: [EncounterRow; 2] = [
    EncounterRow::InteriorCoverage,
    EncounterRow::InteriorPlacement,
];

pub(crate) struct StreamedInteriorDigests<'directory> {
    ordered: [InteriorOrder; 2],
    comparison: StreamedComponentDigests<'directory, 2>,
    roots: ShipLoadoutDigest,
}

impl<'directory> StreamedInteriorDigests<'directory> {
    pub(crate) fn new(directory: &'directory Path) -> Self {
        Self {
            ordered: std::array::from_fn(|_| InteriorOrder::default()),
            comparison: StreamedComponentDigests::for_tables(directory),
            roots: ShipLoadoutDigest::default(),
        }
    }

    pub(crate) fn retain(&mut self, snapshot: &FacilityInteriorPlacementSnapshot) -> TestResult {
        let coverage = &snapshot.coverage;
        let ship = coverage.ship_record_index;
        if coverage.loadout_state == SourceValueState::Present {
            self.roots.append(ship)?;
        }
        let [rooms, placements] = &mut self.ordered;
        let mut origins = InteriorOrigins::default();
        for row in &coverage.rows {
            let proof = origins.retain(EncounterRow::InteriorCoverage, row)?;
            rooms.append(ship, row.loadout_position().is_some(), proof);
        }
        for row in &snapshot.rows {
            let interior = row
                .interior_placement()
                .ok_or("SDK produced a non-Interior row")?;
            if row.ship_record_index().value() != Some(&ship) {
                return Err("the SDK placement has a different Ship from its callback".into());
            }
            let proof = origins.retain(EncounterRow::InteriorPlacement, row)?;
            placements.append(ship, interior.loadout_position().is_some(), proof);
        }
        let inventory = &coverage.source_inventory;
        if !origins.uses.is_empty() {
            capture_inventory(
                inventory,
                origins.uses,
                |id, row| spill(&mut self.comparison.uses, id, row),
                |id, row| spill(&mut self.comparison.sources, id, row),
            )?;
        }
        capture_physical_sources(
            inventory,
            origins.sources.iter().map(String::as_str).collect(),
            |id, row| spill(&mut self.comparison.sources, id, row),
        )
    }

    pub(crate) fn compare(mut self, measurement: &BoundMeasurement) -> TestResult<Counts<2>> {
        let [rooms, placements] = self.ordered;
        self.comparison.rows = [
            rooms.into_ordered(SHAPES[0])?,
            placements.into_ordered(SHAPES[1])?,
        ];
        let loadout_roots = self.roots.finish();
        measurement.verify_streamed_scope(
            StreamedRowPaths::Arrays(PATHS),
            [DatasetAvailability::Available; 2],
            self.comparison,
            None,
            |capture| {
                capture.keep(AVAILABILITY);
                capture.project(LOADOUTS, Projection::ShipLoadoutRows(loadout_roots));
            },
            |catalog| {
                let expected = serde_json::to_value(DatasetAvailability::Available)?;
                if catalog.pointer(AVAILABILITY) != Some(&expected) {
                    return Err("the retained FacilityInstance table is unavailable".into());
                }
                if catalog.pointer(LOADOUTS) != Some(&Value::from(loadout_roots.count())) {
                    return Err("the retained Ship Loadout roots are missing".into());
                }
                Ok(())
            },
        )
    }
}

struct RowProof {
    key: Fingerprint,
    wire: Fingerprint,
}

#[derive(Default)]
struct InteriorOrder {
    direct: BTreeMap<u32, Vec<RowProof>>,
    attached: Vec<RowProof>,
}

impl InteriorOrder {
    fn append(&mut self, ship: u32, attached: bool, row: RowProof) {
        if attached {
            self.attached.push(row);
        } else {
            self.direct.entry(ship).or_default().push(row);
        }
    }

    fn into_ordered(self, shape: EncounterRow) -> TestResult<OrderedRows> {
        let mut ordered = OrderedRows::with_encounter(shape);
        for row in self.direct.into_values().flatten().chain(self.attached) {
            ordered.append_encounter_row(row.key, row.wire)?;
        }
        Ok(ordered)
    }
}

#[derive(Default)]
struct InteriorOrigins {
    uses: HashSet<String>,
    sources: HashSet<String>,
}

impl InteriorOrigins {
    fn retain(&mut self, shape: EncounterRow, row: &impl Serialize) -> TestResult<RowProof> {
        let row = wire_row(row)?;
        let mut references = HashSet::new();
        collect_references(&row, &mut references);
        if references.is_empty() {
            return Err("an Interior row lacks contributing source uses".into());
        }
        self.uses.extend(references.into_iter().map(str::to_owned));
        collect_input_refs(&row, &mut self.sources)?;
        let (key, row) = shape.project(row)?;
        Ok(RowProof {
            key,
            wire: fingerprint(row)?,
        })
    }
}

#[test]
fn interior_comparison_preserves_order_duplicates_and_exact_variant() -> TestResult {
    use serde::de::DeserializeSeed as _;
    use serde_json::json;

    let row = |index: u32| {
        json!({
            "shipRecordIndex": {"state": "present", "value": index},
            "source": {
                "kind": "interior",
                "room": {
                    "owner": {"ownerRecordIndex": {"state": "present", "value": index}},
                    "loadoutPosition": null
                },
                "position": {"state": "present", "value": {
                    "containerOccurrenceOrdinal": 0,
                    "descriptorNodeIndex": 1,
                    "chunkIndex": 0,
                    "entityOrdinal": index
                }}
            }
        })
    };
    let first = row(1);
    let second = row(2);
    let loadout = json!({"source": {"kind": "loadout"}});
    let mut ordering = InteriorOrder::default();
    for (ship, row) in [(2, &second), (1, &first), (2, &second)] {
        let (key, row) = EncounterRow::InteriorPlacement.project(row.clone())?;
        ordering.append(
            ship,
            false,
            RowProof {
                key,
                wire: fingerprint(row)?,
            },
        );
    }
    let expected = ordering
        .into_ordered(EncounterRow::InteriorPlacement)?
        .finish();
    let compare = |wire: Value| {
        expected.deserialize(&mut serde_json::Deserializer::from_str(&wire.to_string()))
    };
    assert_eq!(compare(json!([loadout, first, second, second]))?, json!(3));
    let mut missing_soc = second.clone();
    missing_soc["source"]["position"] = json!({"state": "absent"});
    for rows in [
        json!([first, second]),
        json!([second, first, second]),
        json!([first, second, second, second]),
        json!([first, second, missing_soc]),
        json!([first, {"source": {"kind": "unknown"}}, second, second]),
    ] {
        assert!(compare(rows).is_err());
    }
    Ok(())
}
