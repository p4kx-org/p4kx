// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::catalog_v3::{mineable_composition_definition, mining_process_catalog};
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::source_fields::FieldError;
use crate::{CatalogError, ExportAccountingError, Kind};

pub(crate) fn named_definition<'source>(
    kind: Kind,
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    cancel: Option<&CancelToken>,
) -> Result<(Picked<'source>, Vec<FieldError>), CatalogError> {
    match kind {
        Kind::RefiningProcesses => {
            let (value, notes) =
                mining_process_catalog::read_refining(tree, projection, admission, cancel)?;
            Ok((Picked::Refining(Box::new(value)), notes))
        }
        Kind::MiningGlobalParams => {
            let (value, notes) =
                mining_process_catalog::read_globals(tree, projection, admission, cancel)?;
            Ok((Picked::MiningParams(Box::new(value)), notes))
        }

        Kind::MineableCompositions => {
            let read = mineable_composition_definition::read(tree, projection, admission, cancel)?;
            Ok((Picked::Composition(Box::new(read.value)), read.notes))
        }
        _ => Err(ExportAccountingError::ConflictingSourceDatum.into()),
    }
}
