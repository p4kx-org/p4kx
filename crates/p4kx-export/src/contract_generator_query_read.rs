// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Record, TreeBuilder, Value};

use super::notes::{ContractGeneratorProjectionFailure, ContractGeneratorProjectionNote};
use crate::CatalogError;
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_queries::{CatalogRecordCoverage, CatalogRecordSelection};
use crate::catalog_query_session::checkpoint;
use crate::contract_generator_definition::{self, DefinitionProjection};
use crate::mission_generator::GeneratorDefinition;
use crate::record_observation::RecordObservation;
use crate::source_value_catalog;
use crate::template_field_capture::TemplateFieldCapture;

#[derive(Default)]
pub(super) struct SelectedGenerators<'source> {
    pub(super) rows: Vec<GeneratorDefinition<'source>>,
    pub(super) coverage: CatalogRecordCoverage,
    pub(super) notes: Vec<ContractGeneratorProjectionNote>,
    pub(super) failures: Vec<ContractGeneratorProjectionFailure>,
}

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    input: &OriginalCatalogInput<'_, 'source>,
    selection: CatalogRecordSelection,
    cancel: Option<&CancelToken>,
) -> Result<SelectedGenerators<'source>, CatalogError> {
    let mut result = SelectedGenerators::default();
    let mut trees = TreeBuilder::new();
    selection.for_each(forge, cancel, |record| {
        if !selected(record, &mut result.coverage) {
            return Ok(());
        }
        checkpoint(cancel)?;
        let Ok(tree) = trees.build(record) else {
            result.coverage.unreadable_bodies.push(record.index());
            return Ok(());
        };
        let admission = RecordObservation::capture(input, record)?;
        let scope = input.record_scope(record, &tree)?;
        let mut unreadable = false;
        let indexed = source_value_catalog::read_indexed_arena(&scope, cancel, |site| {
            unreadable |= matches!(site.value(), Value::Error(_));
        })?;
        let capture = TemplateFieldCapture::new(&indexed.facts, input, &admission)?;
        let projected = contract_generator_definition::read(
            input,
            &tree,
            &indexed.facts,
            indexed.source,
            &capture,
        )?;
        result.take(record.index(), projected, unreadable)?;
        Ok(())
    })?;
    Ok(result)
}

fn selected(record: &Record<'_>, coverage: &mut CatalogRecordCoverage) -> bool {
    let Ok(path) = record.record_path() else {
        if matches!(record.type_name(), Ok("ContractGenerator") | Err(_)) {
            coverage.unreadable_headers.push(record.index());
        }
        return false;
    };
    if !contract_generator_definition::matches_path(path, None) {
        return false;
    }
    match record.type_name() {
        Ok("ContractGenerator") => true,
        Ok(_) => false,
        Err(_) => {
            coverage.unreadable_headers.push(record.index());
            false
        }
    }
}

impl<'source> SelectedGenerators<'source> {
    fn take(
        &mut self,
        record_index: u32,
        projected: DefinitionProjection<'source>,
        unreadable: bool,
    ) -> Result<(), CatalogError> {
        let DefinitionProjection {
            definition,
            item_error,
            notes,
        } = projected;
        let incomplete = unreadable
            || item_error.is_some()
            || notes
                .iter()
                .any(|note| note.description().issue.blocks_projection());
        self.notes.extend(
            notes
                .into_iter()
                .map(|note| ContractGeneratorProjectionNote::new(record_index, note)),
        );
        if let Some(error) = item_error {
            let error = match error.downcast::<CatalogError>() {
                Ok(error) => return Err(*error),
                Err(error) => error,
            };
            self.failures
                .push(ContractGeneratorProjectionFailure::new(record_index, error));
        }
        if incomplete {
            self.coverage.incomplete_fields.push(record_index);
        }
        self.rows.push(definition);
        Ok(())
    }
}
