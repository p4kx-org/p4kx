// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, SourcedValue};
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::component::{Component, ComponentKey};
use super::number::Number;
use crate::catalog_queries::equipment::ship_systems::{QuantumDrive, QuantumProfile};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::resource_number::ResourceNumber;

const PATH: &str = "configurations.quantumDrives";

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<StagedCatalogTable, ReducedError> {
    parsed.with_quantum_drives(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability(), snapshot.coverage())?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        for source in snapshot.rows() {
            rows.append(&Drive::read(source)?)?;
        }
        let rows = rows.finish(snapshot.availability())?;
        completeness::rows(PATH, snapshot.rows().len(), rows.len())?;
        Ok(rows)
    })?
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Drive<'read> {
    #[serde(flatten)]
    owner: Component<'read>,
    fuel_requirement: Cell<Number<'read>>,
    jump_range: Cell<Number<'read>>,
    standard: Cell<StandardProfile<'read>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct StandardProfile<'read> {
    drive_speed: Cell<Number<'read>>,
    cooldown_time: Cell<Number<'read>>,
    spool_up_time: Cell<Number<'read>>,
    stage_one_accel_rate: Cell<Number<'read>>,
    stage_two_accel_rate: Cell<Number<'read>>,
    engage_speed: Cell<Number<'read>>,
    calibration_rate: Cell<Number<'read>>,
    min_calibration_requirement: Cell<Number<'read>>,
    max_calibration_requirement: Cell<Number<'read>>,
    calibration_process_angle_limit: Cell<Number<'read>>,
    calibration_warning_angle_limit: Cell<Number<'read>>,
    calibration_delay_in_seconds: Cell<Number<'read>>,
}

impl<'read> Drive<'read> {
    fn read(source: &'read QuantumDrive<'_>) -> Result<Self, ReducedError> {
        ComponentKey::read(source.owner(), PATH)?;
        Ok(Self {
            owner: source.owner().into(),
            fuel_requirement: quantity(source.fuel_requirement())?,
            jump_range: quantity(source.jump_range())?,
            standard: completeness::cell(PATH, source.standard())?
                .try_map(StandardProfile::read)?,
        })
    }
}

impl<'read> StandardProfile<'read> {
    fn read(source: &'read QuantumProfile<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            drive_speed: quantity(source.drive_speed())?,
            cooldown_time: quantity(source.cooldown_time())?,
            spool_up_time: quantity(source.spool_up_time())?,
            stage_one_accel_rate: quantity(source.stage_one_accel_rate())?,
            stage_two_accel_rate: quantity(source.stage_two_accel_rate())?,
            engage_speed: quantity(source.engage_speed())?,
            calibration_rate: quantity(source.calibration_rate())?,
            min_calibration_requirement: quantity(source.min_calibration_requirement())?,
            max_calibration_requirement: quantity(source.max_calibration_requirement())?,
            calibration_process_angle_limit: quantity(source.calibration_process_angle_limit())?,
            calibration_warning_angle_limit: quantity(source.calibration_warning_angle_limit())?,
            calibration_delay_in_seconds: quantity(source.calibration_delay_in_seconds())?,
        })
    }
}

fn quantity(source: &SourcedValue<ResourceNumber>) -> Result<Cell<Number<'_>>, ReducedError> {
    Ok(completeness::cell(PATH, source)?.map(Number::from))
}
