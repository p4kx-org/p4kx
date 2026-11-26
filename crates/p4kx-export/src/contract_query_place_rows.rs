// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, DataForge, Record, RecordTree, TreeBuilder};

use super::super::ContractQueryDiagnostic;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::starmap_object_definition::{self, AuthoredNavigation};
use crate::contract_starmap::StarmapObject;
use crate::export_ledger::ExportLedger;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(super) fn read<'source>(
    forge: &'source DataForge<'source>,
    trees: &mut TreeBuilder<'source>,
    tree: &RecordTree<'source>,
    types: &crate::starmap::TypeNames<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(StarmapObject<'source>, Vec<ContractQueryDiagnostic>), CatalogError> {
    let record = original_record(forge, tree)?;
    let admission = RecordObservation::capture(projection.original(), &record)?;
    let (navigation, notes) = observed_navigation(tree, projection, &admission, cancel)?;
    let mut ledger = ExportLedger::default();
    crate::source_projection_notes::record_dependency(&admission, tree, notes, &mut ledger)?;
    let object = crate::starmap::starmap_object(
        trees,
        tree,
        navigation.name,
        forge,
        types,
        &crate::starmap_containers::Placements::default(),
    );
    let diagnostics = ledger
        .field_diagnostics
        .into_iter()
        .map(ContractQueryDiagnostic::new)
        .collect();
    Ok((object, diagnostics))
}

pub(super) fn publish_name<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(), CatalogError> {
    let record = original_record(forge, tree)?;
    let admission = RecordObservation::capture(projection.original(), &record)?;
    observed_navigation(tree, projection, &admission, cancel)?;
    Ok(())
}

fn original_record<'source>(
    forge: &'source DataForge<'source>,
    tree: &RecordTree<'source>,
) -> Result<Record<'source>, CatalogError> {
    forge
        .record(tree.record_index())
        .ok_or_else(|| ExportAccountingError::ConflictingSourceDatum.into())
}

fn observed_navigation<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(AuthoredNavigation<'source>, Vec<FieldError>), CatalogError> {
    let projection = projection.for_kind(Kind::StarmapObjects);
    starmap_object_definition::read_navigation(tree, &projection, admission, cancel)
}
