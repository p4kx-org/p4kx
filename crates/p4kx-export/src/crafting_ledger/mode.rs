// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForgeError, Filter, Record, RecordTree, SourceUseRole};

use crate::CatalogError;
use crate::catalog_origins::CatalogOrigins;
use crate::export_ledger::ExportLedger;
use crate::quality_catalog::evidence::Evidence;
use crate::quality_catalog::{Input, Retention, Step, scan};
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};

use super::selection::{self, Action, ReadPolicy, Selection};

/// Independent current consumers of the shared crafting selection and canonical read.
#[derive(Clone, Copy)]
pub(crate) enum ReadMode<'context> {
    Interpreted,
    Catalog(&'context CatalogOrigins),
}

impl<'context> ReadMode<'context> {
    pub(super) const fn origins(self) -> Option<&'context CatalogOrigins> {
        match self {
            Self::Interpreted => None,
            Self::Catalog(origins) => Some(origins),
        }
    }

    pub(super) fn select(
        self,
        record: &Record<'_>,
        filter: Option<&Filter>,
    ) -> Result<(Result<Option<Selection>, DataForgeError>, Evidence), CatalogError> {
        let Some(origins) = self.origins() else {
            return Ok((selection::choose(record, filter), Evidence::default()));
        };
        let context = origins.record(record.index());
        let mut inventory = Evidence::default();
        inventory.header(
            &context,
            RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
            true,
        )?;
        let mut failure = None;
        let selected = selection::choose_observed(record, filter, |field, readable| {
            if failure.is_none() {
                failure = inventory.header(&context, field, readable).err();
            }
        });
        if let Some(error) = failure {
            return Err(error);
        }
        Ok((selected, inventory))
    }

    pub(super) fn project<'f>(
        self,
        selected: Selection,
        tree: &RecordTree<'f>,
        ledger: &mut ExportLedger,
        cancel: Option<&CancelToken>,
    ) -> Result<Option<Input<'f>>, CatalogError> {
        let Some(origins) = self.origins() else {
            return Ok(None);
        };
        let retention = match (selected.role, selected.policy) {
            (_, ReadPolicy::SkipAfter) => Retention::Leftover,
            (crate::export_diagnostic::ExportSourceRole::Dependency, _) => Retention::Dependency,
            _ => Retention::Selected,
        };
        match selected.action {
            Action::Distribution => {
                scan::distribution(tree, origins, retention, selected.role, ledger, cancel)
                    .map(Some)
            }
            Action::Quantization => {
                scan::quantization(tree, origins, retention, selected.role, ledger, cancel)
                    .map(Some)
            }
            Action::ResourceLinks => scan::resources(tree, origins, ledger, cancel).map(Some),
            _ => Ok(None),
        }
    }

    pub(super) fn failed<'f>(
        self,
        record: &Record<'f>,
        action: Option<Action>,
    ) -> Result<Step<'f>, CatalogError> {
        let mut step = Step::default();
        let Some(origins) = self.origins() else {
            return Ok(step);
        };
        if !matches!(action, Some(Action::Quantization | Action::ResourceLinks)) {
            return Ok(step);
        }
        let context = origins.record(record.index());
        let mut evidence = Evidence::default();
        evidence.header(
            &context,
            RecordHeaderInput::Field(RecordHeaderField::RecordIndex),
            true,
        )?;
        evidence.keep(context.observe("", SourceUseRole::Attempted)?);
        evidence.complete = false;
        if matches!(action, Some(Action::Quantization)) {
            step.input = Some(Input::UnavailableQuantization {
                guid: record.guid(),
                record_index: record.index(),
                evidence,
            });
        } else {
            step.inventory = evidence;
        }
        Ok(step)
    }
}
