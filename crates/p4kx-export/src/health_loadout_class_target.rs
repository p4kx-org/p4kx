// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::LoadoutFileObservation;
use p4kx_extraction::{Guid, Record, SourceUses, SourceValue};

use super::health_loadout_class_name::LoadoutClassNameReceipt;
use super::health_loadout_graph::ClassXmlSelection;
use super::{OriginalDatum, RecordSubject};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

/// The actual selected Record joined to the original getter that selected its class.
#[derive(Debug, Clone)]
pub(super) struct LoadoutClassTargetReceipt {
    name: LoadoutClassNameReceipt,
    subject: RecordSubject,
    guid: Guid,
    inventory: SourceUses,
}

impl LoadoutClassTargetReceipt {
    pub(super) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        name: LoadoutClassNameReceipt,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        name.check_input(input)?;
        input.record_context(record)?;
        let actual = record.name().map_err(CatalogError::DataForge)?;
        if record.type_name().map_err(CatalogError::DataForge)? != p4kx_assembly::ENTITY_TYPE
            || !actual
                .strip_prefix("EntityClassDefinition.")
                .is_some_and(|actual| actual.eq_ignore_ascii_case(name.value()))
        {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        let subject = RecordSubject::capture(input, record)?;
        Ok(Self {
            name,
            subject,
            guid: record.guid(),
            inventory: input.origins().uses.clone(),
        })
    }

    pub(super) const fn value(&self) -> Guid {
        self.guid
    }

    pub(super) fn original_xml(&self) -> Option<&LoadoutFileObservation> {
        self.name.original_xml()
    }

    pub(super) fn bind_xml(
        &mut self,
        selection: ClassXmlSelection,
    ) -> Result<(), ExportAccountingError> {
        self.name.bind_xml(selection)
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.name.check_input(input)?;
        self.subject.check_input(input)?;
        if !self.inventory.is_same_inventory(&input.origins().uses) {
            return Err(ExportAccountingError::UnboundHealthSubject);
        }
        Ok(())
    }

    pub(super) fn repeat(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<(), CatalogError> {
        self.check_input(input)?;
        input.record_context(record)?;
        let original = self
            .subject
            .key()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        if original.index() != record.index() || self.guid != record.guid() {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        Ok(())
    }

    pub(super) fn retain(&self) -> Result<OriginalDatum<Guid>, CatalogError> {
        let guid = self.subject.retain_guid(&self.inventory)?;
        let (original, known) = guid.contributions();
        let (selected, complete) = self.name.selectors()?;
        let mut inputs = original.to_vec();
        inputs.extend(selected);
        Ok(OriginalDatum::observed(
            SourceValue::Present(self.guid),
            inputs,
            known && complete,
        ))
    }
}
