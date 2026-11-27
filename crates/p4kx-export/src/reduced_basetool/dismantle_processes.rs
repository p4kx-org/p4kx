// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::CancelToken;
use serde::Serialize;

use super::ReducedError;
use super::cell::Cell;
use super::completeness;
use super::number::Number;
use super::record_identity::RecordIdentity;
use crate::catalog_queries::crafting::{
    DismantleBlueprint, DismantleConfiguration, DismantleProcessDefinition,
};
use crate::catalog_queries::{CatalogRecordSelection, ParsedCatalogQueries};
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::guid_text::CatalogGuid;
use crate::resource_integer::ResourceInt64;

pub(super) const PATH: &str = "definitions.dismantleProcesses";

pub(super) struct DismantleProcesses {
    pub(super) definitions: StagedCatalogTable,
    unique: Option<ProcessKey>,
}

struct ProcessKey {
    guid: CatalogGuid,
    record_index: u32,
}

impl DismantleProcesses {
    pub(super) fn unique_record(&self, guid: CatalogGuid) -> Result<u32, ReducedError> {
        self.unique
            .as_ref()
            .filter(|process| process.guid == guid)
            .map(|process| process.record_index)
            .ok_or(ReducedError::IncompleteScope(PATH))
    }
}

pub(super) fn collect(
    parsed: &ParsedCatalogQueries<'_>,
    scratch: &Path,
    cancel: Option<&CancelToken>,
) -> Result<DismantleProcesses, ReducedError> {
    parsed.with_dismantle_processes(CatalogRecordSelection::All, cancel, |snapshot| {
        completeness::inventory(PATH, snapshot.availability, &snapshot.coverage)?;
        let mut rows = StagedCatalogRows::new(scratch, cancel);
        for original in &snapshot.rows {
            rows.append(&Definition::read(original)?)?;
        }
        let definitions = rows.finish(snapshot.availability)?;
        completeness::rows(PATH, snapshot.rows.len(), definitions.len())?;
        let unique = match snapshot.rows.as_slice() {
            [record] => Some(ProcessKey::read(record)?),
            _ => None,
        };
        Ok(DismantleProcesses {
            definitions,
            unique,
        })
    })?
}

impl ProcessKey {
    fn read(original: &DismantleProcessDefinition<'_>) -> Result<Self, ReducedError> {
        let header = original.header();
        Ok(Self {
            guid: completeness::cell(PATH, &header.guid)?
                .value
                .copied()
                .ok_or(ReducedError::IncompleteScope(PATH))?,
            record_index: completeness::cell(PATH, header.record_index())?
                .value
                .copied()
                .ok_or(ReducedError::IncompleteScope(PATH))?,
        })
    }
}

#[derive(Serialize)]
struct Definition<'read> {
    #[serde(flatten)]
    identity: RecordIdentity<'read>,
    blueprint: Cell<Blueprint<'read>>,
}

impl<'read> Definition<'read> {
    fn read(original: &'read DismantleProcessDefinition<'_>) -> Result<Self, ReducedError> {
        let header = original.header();
        completeness::value(PATH, header.class_name())?;
        completeness::value(PATH, header.record_path())?;
        Ok(Self {
            identity: header.into(),
            blueprint: completeness::cell(PATH, original.blueprint())?.try_map(Blueprint::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Blueprint<'read> {
    process_specific_data: Cell<Process<'read>>,
}

impl<'read> Blueprint<'read> {
    fn read(original: &'read DismantleBlueprint<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            process_specific_data: completeness::cell(PATH, original.process_specific_data())?
                .try_map(Process::read)?,
        })
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Process<'read> {
    efficiency: Cell<Number<'read>>,
    time_seconds: Cell<&'read ResourceInt64>,
}

impl<'read> Process<'read> {
    fn read(original: &'read DismantleConfiguration<'_>) -> Result<Self, ReducedError> {
        Ok(Self {
            efficiency: completeness::cell(PATH, original.efficiency())?.map(Number::from),
            time_seconds: completeness::cell(PATH, original.time_seconds())?,
        })
    }
}
