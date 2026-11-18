// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use crate::Kind;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan::projection::SelectedProjection;
use crate::scan_health::ScanError;
use crate::source_fields::FieldError;

impl<'source> SelectedProjection<'_, '_, '_, 'source> {
    pub(super) fn mining_definition(
        &self,
        kind: Kind,
        admission: &RecordObservation<'_, 'source>,
        tree: &RecordTree<'source>,
    ) -> Result<(Picked<'source>, Vec<FieldError>), ScanError> {
        Ok(crate::mining::named_definition(
            kind,
            tree,
            &self.projection,
            admission,
            self.scan.options.cancel,
        )?)
    }
}
