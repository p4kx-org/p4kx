// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_export::catalog_queries::DatasetAvailability;
use serde_json::Value;

use super::{
    BoundMeasurement, ComparedRows, Origins, Projection, RowSelection, SOURCES, SelectedOrigins,
    Table, TestResult, USES, check_table, measurement,
};

const PLACEMENTS: &str = "/placements/facilityInstances";
const PLACEMENTS_ROWS: &str = "/placements/facilityInstances/rows";
const PLACEMENTS_AVAILABILITY: &str = "/placements/facilityInstances/availability";

impl Table {
    pub(crate) fn interior_placements(
        availability: DatasetAvailability,
        ship: u32,
        rows: Vec<Value>,
    ) -> TestResult<Self> {
        Self::from_wire(availability, rows, RowSelection::InteriorPlacement(ship))
    }
}

impl BoundMeasurement {
    pub(crate) fn verify_interior_ship_rows(
        &self,
        path: &str,
        ship: u32,
        expected: &[Value],
        placements: Option<&Table>,
        origins: &Origins,
    ) -> TestResult {
        self.watch.check()?;
        let mut capture = measurement::normal_capture();
        capture.project(
            path,
            Projection::Rows(ComparedRows::bare_array(
                path,
                expected,
                RowSelection::InteriorShip(ship),
            )),
        );
        if let Some(table) = placements {
            if !matches!(table.selection, RowSelection::InteriorPlacement(index) if index == ship) {
                return Err(
                    "the placement and coverage selectors must name the same physical Ship".into(),
                );
            }
            capture.keep(PLACEMENTS_AVAILABILITY);
            capture.project(
                PLACEMENTS_ROWS,
                Projection::Rows(ComparedRows::table(
                    PLACEMENTS,
                    &table.rows,
                    table.selection,
                )),
            );
        }
        capture.project(USES, Projection::Origins(SelectedOrigins(&origins.uses)));
        capture.project(
            SOURCES,
            Projection::Origins(SelectedOrigins(&origins.sources)),
        );
        let catalog = self.read_catalog(&capture)?;
        check_row_array_count(&catalog, path, expected.len())?;
        if let Some(table) = placements {
            check_table(&catalog, PLACEMENTS, table)?;
        }
        origins.verify(&catalog)?;
        self.watch.check()?;
        Ok(())
    }
}

pub(super) fn check_row_array_count(catalog: &Value, path: &str, expected: usize) -> TestResult {
    if catalog.pointer(path) != Some(&Value::from(expected)) {
        return Err(format!("missing or incomplete Catalog row array at {path}").into());
    }
    Ok(())
}
