// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{VehicleAttributeEvidence, VehiclePortSelection, VehicleXmlSite};
use p4kx_extraction::{
    Archive, SharedFieldOrigin, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

use super::port_vehicle_calls::PortVehicleCall;
use super::vehicle_stage::Contributions;
use super::{Capture, State};
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, OriginalDatum, VehicleFileSelection,
};
use crate::{CatalogError, ExportAccountingError};

struct VehicleFiles<'scope> {
    state: &'scope State,
    entries: Option<EntryCapture<'scope>>,
    call: Option<&'scope PortVehicleCall>,
}

pub(super) struct VehiclePortSources<'scope> {
    sources: &'scope SourceUses,
    files: VehicleFiles<'scope>,
    shared: SharedFieldOrigin,
}

impl<'scope> VehiclePortSources<'scope> {
    pub(super) fn read(
        capture: &'scope Capture<'_>,
        state: &'scope State,
        selected: &VehiclePortSelection<'_>,
        name: Option<&AuthoredEntryPath>,
        archive: Option<&Archive>,
        record_index: Option<u32>,
    ) -> Result<Self, CatalogError> {
        let files = VehicleFiles {
            entries: capture.input.map(EntryCapture::for_input),
            state,
            call: record_index.and_then(|index| state.port_vehicles.call(index)),
        };
        let (inputs, known) = name.map_or_else(
            || Ok((Vec::new(), false)),
            AuthoredEntryPath::selector_inputs,
        )?;
        let mut proof = Contributions::new(inputs, known && selected.is_complete());
        files.select_files(archive, &mut proof)?;
        for site in selected.selectors() {
            files.retain(site, SourceUseRole::Selector, &mut proof)?;
        }
        let sources = &capture.name_sources;
        Ok(Self {
            sources,
            files,
            shared: proof.shared(sources),
        })
    }

    pub(super) fn value<T>(
        &self,
        value: SourceValue<T>,
        evidence: Option<&VehicleAttributeEvidence>,
        role: SourceUseRole,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let mut local = Contributions::default();
        if let Some(evidence) = evidence {
            let role = if matches!(value, SourceValue::Absent) {
                SourceUseRole::Selector
            } else {
                role
            };
            self.files.retain(evidence.original(), role, &mut local)?;
            for selector in evidence.selectors() {
                self.files
                    .retain(selector, SourceUseRole::Selector, &mut local)?;
            }
        } else {
            local.incomplete();
        }
        let (uses, complete) = local.into_parts();
        Ok(self
            .sources
            .value_with_shared_origin(value, &self.shared, &uses, complete)?)
    }
}

impl VehicleFiles<'_> {
    fn select_files(
        &self,
        archive: Option<&Archive>,
        proof: &mut Contributions,
    ) -> Result<(), CatalogError> {
        let (Some(entries), Some(call), Some(archive)) = (&self.entries, self.call, archive) else {
            proof.incomplete();
            return Ok(());
        };
        entries.check_archive(archive.input_origin())?;
        proof.add(call.caller.selection()?);
        if !call.modification_observed {
            proof.incomplete();
        }
        let directory = if call.files.iter().any(VehicleFileSelection::needs_directory) {
            entries
                .source_uses()
                .observe_root_directory_paths(archive)?
        } else {
            None
        };
        let directory_complete =
            directory.is_some() && crate::export_archive_health::inventory(archive).is_none();
        for file in &call.files {
            proof.add(file.attempt(entries, directory.as_ref(), directory_complete)?);
        }
        Ok(())
    }

    fn retain(
        &self,
        site: &VehicleXmlSite,
        role: SourceUseRole,
        proof: &mut Contributions,
    ) -> Result<(), CatalogError> {
        match self.site(site, role)? {
            Some(input) => proof.add(input),
            None => proof.incomplete(),
        }
        Ok(())
    }

    fn site(
        &self,
        site: &VehicleXmlSite,
        role: SourceUseRole,
    ) -> Result<Option<OriginalDatum<()>>, CatalogError> {
        let Some(entries) = &self.entries else {
            return Ok(None);
        };
        if !self.state.files.contains(site.file()) {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        if site.file().source().entry_origin().is_none() {
            return Ok(None);
        }
        if let Some(call) = self.call {
            let file = call
                .files
                .iter()
                .find(|file| file.matches(site.file(), &call.caller))
                .ok_or(ExportAccountingError::UnobservedFailure)?;
            Ok(Some(file.site(entries, site, role)?))
        } else {
            Ok(Some(entries.vehicle_xml_site(site, role)?))
        }
    }
}
