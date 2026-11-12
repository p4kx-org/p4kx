// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::sync::Arc;

use p4kx_assembly::LoadoutFileObservation;
use p4kx_extraction::{Guid, Record, SourceValue, StructValue};

use super::health_loadout_class_name::LoadoutClassNameReceipt;
use super::health_loadout_class_target::LoadoutClassTargetReceipt;
use super::health_loadout_graph::ClassXmlSelection;
use super::health_reference_receipt::{ReferenceDatum, ReferenceFrame};
use super::{OriginalDatum, OriginalRecordScope, OriginalValueFact};
use crate::build::original_input::OriginalCatalogInput;
use crate::{CatalogError, ExportAccountingError};

/// A target GUID paired with its actual authored Reference or selected class operation.
#[derive(Debug, Clone)]
pub(crate) struct LoadoutTargetReceipt {
    source: TargetSource,
}

#[derive(Debug, Clone)]
enum TargetSource {
    Reference {
        frame: Arc<ReferenceFrame>,
        guid: ReferenceDatum<Guid>,
    },
    Class(LoadoutClassTargetReceipt),
}

/// The exact original read frame borrowed by the SDK Reference getter callback.
pub(crate) struct LoadoutTargetCapture<'scope, 'read, 'source> {
    original_scope: &'scope OriginalRecordScope<'read, 'source>,
    frame: Arc<ReferenceFrame>,
}

impl<'scope, 'read, 'source> LoadoutTargetCapture<'scope, 'read, 'source> {
    pub(crate) fn from_paths(paths: &super::EntryPathCapture<'scope, 'read, 'source>) -> Self {
        Self {
            original_scope: paths.scope(),
            frame: paths.reference_frame(),
        }
    }

    pub(crate) fn value_fact(
        &self,
        parent: &StructValue<'source>,
        fact: &OriginalValueFact<'scope, 'read, 'source>,
        ancestors: &[&OriginalValueFact<'scope, 'read, 'source>],
    ) -> Result<LoadoutTargetReceipt, CatalogError> {
        if !std::ptr::eq(fact.scope(), self.original_scope) {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        }
        self.frame.check_scope(fact.scope())?;
        super::health_value_chain::parent(parent, fact, ancestors)?;
        let guid = super::health_reference_target::guid_for_fact(fact)?;
        Ok(LoadoutTargetReceipt {
            source: TargetSource::Reference {
                frame: Arc::clone(&self.frame),
                guid,
            },
        })
    }
}

impl LoadoutTargetReceipt {
    pub(crate) fn selected_class(
        input: &OriginalCatalogInput<'_, '_>,
        name: LoadoutClassNameReceipt,
        record: &Record<'_>,
    ) -> Result<Self, CatalogError> {
        Ok(Self {
            source: TargetSource::Class(LoadoutClassTargetReceipt::read(input, name, record)?),
        })
    }

    pub(crate) const fn value(&self) -> Result<Guid, ExportAccountingError> {
        match &self.source {
            TargetSource::Reference { guid, .. } => match guid.value() {
                SourceValue::Present(guid) => Ok(*guid),
                _ => Err(ExportAccountingError::ConflictingSourceDatum),
            },
            TargetSource::Class(receipt) => Ok(receipt.value()),
        }
    }

    pub(crate) fn original_class_xml(
        &self,
    ) -> Result<Option<&LoadoutFileObservation>, ExportAccountingError> {
        match &self.source {
            TargetSource::Class(receipt) => Ok(receipt.original_xml()),
            TargetSource::Reference { .. } => Err(ExportAccountingError::ConflictingSourceDatum),
        }
    }

    pub(crate) fn bind_class_xml(
        &mut self,
        selection: ClassXmlSelection,
    ) -> Result<(), ExportAccountingError> {
        match &mut self.source {
            TargetSource::Class(receipt) => receipt.bind_xml(selection),
            TargetSource::Reference { .. } => Err(ExportAccountingError::ConflictingSourceDatum),
        }
    }

    pub(crate) fn repeat_class(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
        record: &Record<'_>,
    ) -> Result<(), CatalogError> {
        match &self.source {
            TargetSource::Class(receipt) => receipt.repeat(input, record),
            TargetSource::Reference { .. } => {
                Err(ExportAccountingError::ConflictingSourceDatum.into())
            }
        }
    }

    pub(super) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        match &self.source {
            TargetSource::Reference { frame, .. } => frame.check_input(input),
            TargetSource::Class(receipt) => receipt.check_input(input),
        }
    }

    pub(crate) fn retained_for(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<OriginalDatum<Guid>, CatalogError> {
        self.check_input(input)?;
        self.retain()
    }

    pub(super) fn retain(&self) -> Result<OriginalDatum<Guid>, CatalogError> {
        match &self.source {
            TargetSource::Reference { frame, guid } => guid.clone().finish(frame),
            TargetSource::Class(receipt) => receipt.retain(),
        }
    }
}

#[cfg(test)]
#[path = "health_loadout_target_tests.rs"]
mod tests;
