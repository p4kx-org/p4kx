// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use super::SelectedProjection;
use crate::catalog_v3::industry_modules::crafter_definition::{self, PreparedCrafter};
use crate::export_diagnostic::ExportSourceRole;
use crate::export_ledger::ExportLedger;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, Kind};

impl<'source> SelectedProjection<'_, '_, '_, 'source> {
    pub(super) fn with_crafter(
        &self,
        picked: Option<Picked<'source>>,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Picked<'source>>, CatalogError> {
        if !crafter_definition::candidate(self.kind) {
            return Ok(picked);
        }
        let Some(station) = self.crafter(admission, tree, ledger)? else {
            return Ok(picked);
        };
        Ok(Some(match picked {
            Some(primary) => Picked::Crafted(Box::new(primary), station),
            None => Picked::Crafter(station),
        }))
    }

    fn crafter(
        &self,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
        ledger: &mut ExportLedger,
    ) -> Result<Option<Box<PreparedCrafter<'source>>>, CatalogError> {
        let Some((station, notes)) = crafter_definition::prepare(
            tree,
            self.projection.original(),
            admission,
            self.scan.options.cancel,
        )?
        else {
            return Ok(None);
        };
        if crafter_definition::selection_incomplete(&station) {
            ledger.incomplete_inventory(Kind::Crafters);
        }
        crate::source_projection_notes::record(
            Kind::Crafters,
            ExportSourceRole::Selected,
            admission,
            tree,
            notes,
            ledger,
        )?;
        Ok(Some(Box::new(station)))
    }
}
