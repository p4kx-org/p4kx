// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::EntityNode;
use p4kx_extraction::{FieldOrigin, SourceUseRole};

use crate::loadout::ledger::Capture;
use crate::record_header_field::{RecordHeaderField, RecordHeaderInput};
use crate::{CatalogError, ExportAccountingError};

/// The last check reached by the canonical grid admission predicate.
#[derive(Clone, Copy)]
pub(super) enum CargoAdmission {
    OtherType,
    TemplateClass,
    TemplatePath,
    Grid,
}

impl CargoAdmission {
    pub(super) fn of(node: &EntityNode) -> Self {
        if !node.attach_type().eq_ignore_ascii_case("CargoGrid") {
            Self::OtherType
        } else if node
            .class_name()
            .to_ascii_lowercase()
            .ends_with("_template")
        {
            Self::TemplateClass
        } else if node
            .record_path()
            .to_ascii_lowercase()
            .ends_with("_cargogrid_template")
        {
            Self::TemplatePath
        } else {
            Self::Grid
        }
    }

    pub(super) fn observe(
        self,
        capture: &Capture<'_>,
        node: &EntityNode,
        origins: &mut Vec<FieldOrigin>,
    ) -> Result<(), CatalogError> {
        let record_index = node
            .record_index()
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        origins.push(capture.cargo_type_origin(record_index)?);
        if !matches!(self, Self::Grid) {
            let (attachment, _) =
                capture.attachment_identity(node.guid(), record_index, node.attachment_target())?;
            origins.push(attachment.target_record_index().origin().clone());
        }
        if !matches!(self, Self::OtherType) {
            origins.push(self.observe_header(capture, node, record_index)?);
        }
        Ok(())
    }

    fn observe_header(
        self,
        capture: &Capture<'_>,
        node: &EntityNode,
        record_index: u32,
    ) -> Result<FieldOrigin, CatalogError> {
        let (input, _) = capture.cargo_context()?;
        let record = input
            .forge()
            .record(record_index)
            .ok_or(ExportAccountingError::UnobservedFailure)?;
        if record.guid() != node.guid()
            || record.name().map_err(CatalogError::DataForge)? != node.name()
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let context = input.record_context(&record)?;
        let mut uses =
            vec![context.observe_header(RecordHeaderField::RecordName, SourceUseRole::Selector)?];
        if matches!(self, Self::TemplatePath | Self::Grid) {
            if record.record_path().map_err(CatalogError::DataForge)? != node.record_path() {
                return Err(ExportAccountingError::ConflictingSourceDatum.into());
            }
            context.append_header_input(
                RecordHeaderInput::NormalizedPath,
                SourceUseRole::Selector,
                &mut uses,
            )?;
        }
        Ok(context.sources().origin(&uses, true))
    }
}
