// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_extraction::RecordTree;

use super::Scan;
use crate::mission_kinds::MissionPick;
use crate::picked::Picked;
use crate::record_observation::RecordObservation;
use crate::scan_health::ScanError;
use crate::source_fields::FieldError;

impl<'f> Scan<'_, '_, 'f> {
    pub(super) fn mission_type_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let definition = crate::mission_data::mission_type(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        Ok((Picked::Mission(MissionPick::Type(definition)), Vec::new()))
    }

    pub(super) fn mission_item_definition(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let (definition, notes) = crate::catalog_v3::mission_sources::item_definition::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        Ok((
            Picked::Mission(MissionPick::Item(Box::new(definition))),
            notes,
        ))
    }

    pub(super) fn module_declaration(
        &self,
        admission: &RecordObservation<'_, 'f>,
        tree: &RecordTree<'f>,
    ) -> Result<(Picked<'f>, Vec<FieldError>), ScanError> {
        let (definition, notes) = crate::catalog_v3::mission_sources::module_declaration::read(
            tree,
            self.projection,
            admission,
            self.options.cancel,
        )?;
        Ok((
            Picked::Mission(MissionPick::ModuleDeclaration(Box::new(definition))),
            notes,
        ))
    }
}
