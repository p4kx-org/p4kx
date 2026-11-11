// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::ops::ControlFlow;

use p4kx_assembly::{LoadoutClassSelection, LoadoutXmlClassUse, XmlLoadoutClassSite};
use p4kx_extraction::{Guid, Record};

use crate::catalog_read::original::{ExampleBodyReceipt, LoadoutClassNameReceipt};
use crate::health_diagnostic_observation::ExampleObservation;
use crate::record_observation::RecordObservation;
use crate::{CatalogError, ExportAccountingError};

use super::Capture;

impl Capture<'_> {
    pub(super) fn retain_class(
        &self,
        selection: &LoadoutClassSelection,
        name: LoadoutClassNameReceipt,
    ) -> Result<(), CatalogError> {
        self.lock()?.classes.getter(selection, name)?;
        Ok(())
    }

    pub(super) fn class_xml(
        &self,
        selection: &LoadoutClassSelection,
        site: &XmlLoadoutClassSite<'_>,
    ) -> ControlFlow<()> {
        let Some(input) = self.input else {
            return ControlFlow::Continue(());
        };
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        let result = (|| {
            if site.observation().read_outcome().is_err()
                || !self.lock()?.files.contains(site.observation())
            {
                return Err(ExportAccountingError::UnobservedFailure.into());
            }
            let name = LoadoutClassNameReceipt::xml(input, site)?;
            self.retain_class(selection, name)
        })();
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn class_selected(
        &self,
        selection: &LoadoutClassSelection,
        record: &Record<'_>,
    ) -> ControlFlow<()> {
        let Some(input) = self.input else {
            return ControlFlow::Continue(());
        };
        if self.stopped() {
            return self.fail(CatalogError::Cancelled);
        }
        let result = self
            .lock()
            .map_err(CatalogError::from)
            .and_then(|mut state| state.classes.selected(input, selection, record));
        match result {
            Ok(()) => ControlFlow::Continue(()),
            Err(error) => self.fail(error),
        }
    }

    pub(super) fn class_example(
        &self,
        admission: &RecordObservation<'_, '_>,
        selection: &LoadoutClassSelection,
        guid: Guid,
        xml_use: Option<&LoadoutXmlClassUse>,
    ) -> Result<ExampleObservation, CatalogError> {
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let target = {
            let state = self.lock()?;
            let mut target = state.classes.get(selection, guid)?;
            match (target.original_class_xml()?, xml_use) {
                (Some(original), Some(usage)) => {
                    let selected = state.graph.selected_xml_use(usage, original)?;
                    target.bind_class_xml(selected)?;
                }
                (None, None) => {}
                _ => return Err(ExportAccountingError::ConflictingSourceDatum.into()),
            }
            drop(state);
            target
        };
        let body = ExampleBodyReceipt::loadout_target(input, admission.subject().clone(), target)?;
        let (field, source_type, target_guid) = body.description()?;
        Ok(ExampleObservation::captured(
            admission.subject().clone(),
            field.as_deref(),
            source_type.as_deref(),
            target_guid,
            body,
        )?)
    }
}
