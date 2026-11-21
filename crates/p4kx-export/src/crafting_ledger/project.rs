// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::{CancelToken, Filter, RecordTree};

use crate::catalog_projection::CatalogProjection;
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::quality_catalog::Step;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

use super::ReadMode;
use super::fault::Fault;
use super::projection::{self, Piece, ProjectError};
use super::read::Tracked;
use super::selection::{Action, Selection};

fn read_blueprint<'source>(
    tree: &RecordTree<'source>,
    projection: &CatalogProjection<'_, 'source>,
    admission: &RecordObservation<'_, 'source>,
    role: ExportSourceRole,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Piece<'source>, CatalogError> {
    let found = crate::blueprint_catalog_read::read(
        tree,
        &projection.for_kind(Kind::Blueprints),
        admission,
        cancel,
    )?;
    crate::source_projection_notes::record(
        Kind::Blueprints,
        role,
        admission,
        tree,
        found.notes,
        ledger,
    )?;
    Ok(Piece::CanonicalBlueprint(Box::new(found.value)))
}

fn read_selected_projection<'source>(
    selected: Selection,
    admission: &RecordObservation<'_, 'source>,
    tree: &RecordTree<'source>,
    ledger: &mut ExportLedger,
    cancel: Option<&CancelToken>,
) -> Result<Option<Piece<'source>>, ProjectError> {
    let projected = projection::project(selected.action, tree, cancel)?;
    if let Some(kind) = selected.action.kind() {
        crate::source_projection_notes::record(
            kind,
            selected.role,
            admission,
            tree,
            projected.notes,
            ledger,
        )
        .map_err(ProjectError::Catalog)?;
    }
    Ok(projected.piece)
}

pub(super) fn tree<'f>(
    admission: &RecordObservation<'_, 'f>,
    tree: &RecordTree<'f>,
    selected: Selection,
    mode: ReadMode<'_, 'f>,
    cancel: Option<&CancelToken>,
    filter: Option<&Filter>,
) -> Result<Tracked<'f>, CatalogError> {
    let mut ledger = crate::export_field_walk::inspect_observed(
        mode.input(),
        tree,
        admission,
        selected.action.kind(),
        selected.role,
        cancel,
    )?;
    let input = mode.project(selected, admission, tree, &mut ledger, cancel)?;
    let gameplay = mode.property(selected, admission, tree, &mut ledger, cancel)?;
    let pool = mode.pool(selected, admission, tree, &mut ledger, cancel)?;
    let piece = if pool.is_some() {
        pool
    } else if input.is_some() || gameplay.is_some() {
        None
    } else if let Some(projection) = mode.projection()
        && selected.action == Action::Blueprint
    {
        Some(read_blueprint(
            tree,
            projection,
            admission,
            selected.role,
            &mut ledger,
            cancel,
        )?)
    } else {
        match read_selected_projection(selected, admission, tree, &mut ledger, cancel) {
            Ok(piece) => piece,
            Err(ProjectError::Catalog(error)) => return Err(error),
            Err(ProjectError::Source(error)) => {
                let fault = Fault::Projection(&error);
                return super::read::failure(selected, admission, fault, ledger, filter);
            }
        }
    };
    let rules = mode.blueprint_rules(
        selected,
        admission,
        tree,
        piece.as_ref(),
        &mut ledger,
        cancel,
    )?;
    ledger.observe(selected.observation(admission, selected.successful_disposition())?);
    Ok(Tracked {
        piece,
        ledger,
        gameplay,
        blueprint_rules: rules.rules,
        global: rules.global,
        dismantle: rules.dismantle,
        quality: Step {
            input,
            ..Step::default()
        },
    })
}
