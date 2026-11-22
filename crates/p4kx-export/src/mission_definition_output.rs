// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::path::Path;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};
use serde::Serialize;

use crate::catalog_row::CatalogRow;
use crate::catalog_rows::staged::{StagedCatalogRows, StagedCatalogTable};
use crate::catalog_table::DatasetAvailability;
use crate::template_number_capture::TemplateNumbers;
use crate::{CatalogError, Kind};

/// The physical identity and numerical evidence of a canonical mission definition.
pub(crate) struct DefinitionSource<'source> {
    pub(crate) guid: Guid,
    pub(crate) path: &'source str,
    numbers: TemplateNumbers,
}

impl<'source> DefinitionSource<'source> {
    pub(crate) const fn new(tree: &RecordTree<'source>, numbers: TemplateNumbers) -> Self {
        Self {
            guid: tree.guid(),
            path: tree.record_path(),
            numbers,
        }
    }
}

/// The accounting proof obtained from one serialized canonical definition.
pub(crate) struct DefinitionProof {
    pub(crate) row: CatalogRow,
    pub(crate) numbers: TemplateNumbers,
}

/// Ordered canonical definitions and their independently retained accounting.
pub(crate) struct MissionDefinitionOutput {
    pub(crate) table: StagedCatalogTable,
    pub(crate) proof: Vec<DefinitionProof>,
}

impl MissionDefinitionOutput {
    pub(crate) const fn empty() -> Self {
        Self {
            table: StagedCatalogTable::empty(DatasetAvailability::Absent),
            proof: Vec::new(),
        }
    }
}

/// Stages canonical mission definitions independently of their contextual projections.
pub(crate) struct MissionDefinitionWriter<'stage> {
    kind: Kind,
    rows: StagedCatalogRows<'stage>,
    proof: Vec<DefinitionProof>,
}

impl<'stage> MissionDefinitionWriter<'stage> {
    pub(crate) const fn new(
        kind: Kind,
        scratch: &'stage Path,
        cancel: Option<&'stage CancelToken>,
    ) -> Self {
        Self {
            kind,
            rows: StagedCatalogRows::new(scratch, cancel),
            proof: Vec::new(),
        }
    }

    pub(crate) fn append<'source, T: Serialize>(
        &mut self,
        definition: T,
        identity: impl FnOnce(T) -> DefinitionSource<'source>,
    ) -> Result<(), CatalogError> {
        self.rows.append(&definition)?;
        let source = identity(definition);
        self.proof.push(DefinitionProof {
            row: CatalogRow {
                kind: self.kind,
                guid: source.guid,
                record_path: source.path.to_owned(),
                record_index: None,
            },
            numbers: source.numbers,
        });
        Ok(())
    }

    pub(crate) fn finish(
        self,
        forge: &DataForge<'_>,
    ) -> Result<MissionDefinitionOutput, CatalogError> {
        let availability = if self.proof.is_empty() {
            DatasetAvailability::from_source_declarations(forge, |name| {
                name == self.kind.records().type_name
            })
        } else {
            DatasetAvailability::Available
        };
        Ok(MissionDefinitionOutput {
            table: self.rows.finish(availability)?,
            proof: self.proof,
        })
    }
}
