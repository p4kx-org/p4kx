// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::cell::RefCell;

use p4kx_extraction::{CancelToken, DataForge, Guid, RecordTree};

use super::authored_fields::Observed;
use crate::canonical_record_header::CanonicalRecordHeader;
use crate::catalog_number_capture::CapturedNumber;
use crate::catalog_projection::CatalogProjection;
use crate::catalog_read::original::OriginalFlowFields;
use crate::catalog_table::CatalogTable;
use crate::guid_text::CatalogGuid;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::template_field_capture::TemplateFieldCapture;
use crate::{CatalogError, Kind};

/// An authored definition with its physical identity and numeric receipts.
pub(crate) struct PreparedDefinition<'source, T> {
    definition: T,
    guid: Guid,
    index: u32,
    path: &'source str,
    numbers: Vec<CapturedNumber>,
}

impl<'source, T> PreparedDefinition<'source, T> {
    /// Projects a physical Record through the exact admitted source scope.
    pub(super) fn read(
        tree: &RecordTree<'source>,
        projection: &CatalogProjection<'_, 'source>,
        admission: &RecordObservation<'_, 'source>,
        cancel: Option<&CancelToken>,
        project: impl FnOnce(
            CanonicalRecordHeader<'source>,
            Observed<'_, '_, '_, 'source>,
            &CatalogProjection<'_, 'source>,
            &mut Vec<FieldError>,
        ) -> Result<T, CatalogError>,
    ) -> Result<(Self, Vec<FieldError>), CatalogError> {
        let input = projection.original();
        let scope = input.record_scope(admission.record(), tree)?;
        let originals = OriginalFlowFields::read(&scope, cancel)?;
        let capture = TemplateFieldCapture::new(&originals, input, admission)?;
        let numbers = RefCell::new(Vec::new());
        let fields = Observed::definition(&originals, input, &capture, &numbers);
        let mut notes = Vec::new();
        let source = CanonicalRecordHeader::read(tree, projection.origins())?;
        let definition = project(source, fields, projection, &mut notes)?;
        Ok((
            Self {
                definition,
                guid: tree.guid(),
                index: tree.record_index(),
                path: tree.record_path(),
                numbers: numbers.into_inner(),
            },
            notes,
        ))
    }

    pub(super) fn order(rows: &mut [Self]) {
        rows.sort_by_cached_key(|row| (CatalogGuid(row.guid).text(), row.index));
    }

    pub(super) fn table(
        forge: &DataForge<'_>,
        source_type: &str,
        rows: Vec<Self>,
    ) -> CatalogTable<T> {
        CatalogTable::new(
            forge,
            &[source_type],
            rows.into_iter().map(|row| row.definition).collect(),
        )
    }

    pub(super) fn sources<'rows>(
        kind: Kind,
        rows: &'rows [Self],
    ) -> impl Iterator<Item = (Kind, Guid, &'source str, &'rows [CapturedNumber])> {
        rows.iter()
            .map(move |row| (kind, row.guid, row.path, row.numbers.as_slice()))
    }
}
