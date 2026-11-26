// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{
    LoadoutDocumentKind, LoadoutError, LoadoutFailurePath, LoadoutFileError,
    VehicleAttributeEvidence, VehicleImplementation, VehicleItemType, VehiclePortSelection,
    VehicleXmlSite,
};
use p4kx_extraction::{
    Archive, SharedFieldOrigin, SourceUseRole, SourceUses, SourceValue, SourcedValue,
};

#[path = "vehicle_structure_binding.rs"]
pub(crate) mod structure_binding;

use super::port_vehicle_calls::PortVehicleCall;
use super::vehicle_stage::Contributions;
use super::{Capture, State};
use crate::catalog_read::original::{
    AuthoredEntryPath, EntryCapture, OriginalDatum, VehicleFileSelection,
};
use crate::contract_loadout::{PortSubTypes, PortType};
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
        self.retain_attribute(evidence, role, &mut local)?;
        self.sourced_value(value, local)
    }

    fn retain_attribute(
        &self,
        evidence: Option<&VehicleAttributeEvidence>,
        role: SourceUseRole,
        local: &mut Contributions,
    ) -> Result<(), CatalogError> {
        if let Some(evidence) = evidence {
            let role = if evidence.original().checked_absence() {
                SourceUseRole::Selector
            } else {
                role
            };
            self.files.retain(evidence.original(), role, local)?;
            for selector in evidence.selectors() {
                self.files
                    .retain(selector, SourceUseRole::Selector, local)?;
            }
        } else {
            local.incomplete();
        }
        Ok(())
    }

    fn sourced_value<T>(
        &self,
        value: SourceValue<T>,
        local: Contributions,
    ) -> Result<SourcedValue<T>, CatalogError> {
        let (uses, complete) = local.into_parts();
        Ok(self
            .sources
            .value_with_shared_origin(value, &self.shared, &uses, complete)?)
    }

    pub(super) fn item_types(
        &self,
        kinds: &[VehicleItemType],
        position: Option<&VehicleXmlSite>,
    ) -> Result<SourcedValue<Vec<PortType>>, CatalogError> {
        let mut members = Contributions::new(Vec::new(), false);
        if let Some(position) = position {
            self.files
                .retain(position, SourceUseRole::Selector, &mut members)?;
        }
        let items = kinds
            .iter()
            .map(|kind| self.item_type(kind, &mut members))
            .collect::<Result<_, _>>()?;
        self.sourced_value(SourceValue::Present(items), members)
    }

    fn item_type(
        &self,
        kind: &VehicleItemType,
        members: &mut Contributions,
    ) -> Result<PortType, CatalogError> {
        let mut local = Contributions::default();
        for site in kind.selection_sites() {
            self.files
                .retain(site, SourceUseRole::Selector, &mut local)?;
        }
        let mut name = local.clone();
        let (item_type, evidence) = kind.item_type_source();
        self.retain_attribute(evidence, SourceUseRole::Selector, members)?;
        members.extend(local.clone());
        self.retain_attribute(evidence, SourceUseRole::Payload, &mut name)?;
        let evidence = kind.subtypes_evidence();
        self.retain_attribute(evidence, SourceUseRole::Conversion, &mut local)?;
        let state = if evidence.is_some_and(|evidence| evidence.original().checked_absence()) {
            SourceValue::Absent
        } else {
            SourceValue::Present(())
        };
        let sub_types = self.sourced_value(state, local)?;
        let member = sub_types.clone();
        Ok(PortType {
            r#type: self.sourced_value(SourceValue::Present(item_type.to_owned()), name)?,
            sub_types: sub_types.map(|()| PortSubTypes::shared(kind.subtypes().to_vec(), member)),
        })
    }
}

impl Capture<'_> {
    pub(crate) fn port_visibility(
        &self,
        code: &'static str,
        xml: Option<&VehiclePortSelection<'_>>,
        name: Option<&AuthoredEntryPath>,
        flags: Option<&SourcedValue<Vec<String>>>,
        archive: Option<&Archive>,
        record_index: Option<u32>,
    ) -> Result<SourcedValue<&'static str>, CatalogError> {
        let value = SourceValue::Present(code);
        let Some(xml) = xml else {
            let visibility = self.name_sources.value(value, &[], false);
            return match flags {
                Some(flags) => Ok(self
                    .name_sources
                    .add_value_origins(visibility, &[flags.origin()])?),
                None => Ok(visibility),
            };
        };
        let state = self.lock()?;
        let source = VehiclePortSources::read(self, &state, xml, name, archive, record_index)?;
        let visibility = source.value(
            value,
            xml.port().flags_evidence(),
            SourceUseRole::Conversion,
        );
        drop(state);
        visibility
    }

    pub(crate) fn paint_missing_vehicle_survey(
        &self,
        record_index: u32,
        missing: &LoadoutFailurePath,
        archive: &Archive,
    ) -> Result<p4kx_extraction::FieldOrigin, CatalogError> {
        let state = self.lock()?;
        let input = self
            .input
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let call = state.missing_port_vehicle(input, record_index, missing)?;
        let files = VehicleFiles {
            entries: Some(EntryCapture::for_input(input)),
            state: &state,
            call: call.as_ref(),
        };
        let mut proof = Contributions::default();
        files.select_files(Some(archive), &mut proof)?;
        drop(state);
        Ok(proof.origin(&self.name_sources))
    }

    /// Binds the scoped vehicle files, effective XML selectors and paint attributes.
    pub(crate) fn paint_vehicle_survey(
        &self,
        record_index: u32,
        vehicle: &VehicleImplementation,
        archive: &Archive,
    ) -> Result<p4kx_extraction::FieldOrigin, CatalogError> {
        let state = self.lock()?;
        let files = VehicleFiles {
            entries: self.input.map(EntryCapture::for_input),
            state: &state,
            call: state.port_vehicles.call(record_index),
        };
        let mut proof = Contributions::default();
        let directory_complete = files.select_files(Some(archive), &mut proof)?;
        let explicit_patch_absent =
            files.verified_explicit_patch_absence(vehicle, directory_complete);
        files.paint_contents(&mut proof)?;
        for selection in vehicle.item_ports_with_selection() {
            let port = selection.port();
            if !port
                .types()
                .iter()
                .any(|kind| crate::paint::is_paint_type(kind))
            {
                continue;
            }
            let Some(item_type) = port
                .item_types()
                .iter()
                .find(|kind| crate::paint::is_paint_type(kind.item_type()))
            else {
                proof.incomplete();
                continue;
            };
            if !(selection.is_complete()
                || explicit_patch_absent && selection.missing_explicit_patch_is_only_gap())
            {
                proof.incomplete();
            }
            for site in selection.selectors().chain(item_type.selection_sites()) {
                files.retain(site, SourceUseRole::Selector, &mut proof)?;
            }
            let (item_type, type_origin) = item_type.item_type_source();
            for (value, evidence) in [
                (Some(item_type), type_origin),
                selection.port().port_tags_source(),
                selection.port().required_tags_source(),
            ] {
                let Some(evidence) = evidence else {
                    if value.is_some() {
                        proof.incomplete();
                    }
                    continue;
                };
                files.retain(evidence.original(), SourceUseRole::Conversion, &mut proof)?;
                for site in evidence.selectors() {
                    files.retain(site, SourceUseRole::Selector, &mut proof)?;
                }
            }
        }
        drop(state);
        Ok(proof.origin(&self.name_sources))
    }
}

impl VehicleFiles<'_> {
    fn paint_contents(&self, proof: &mut Contributions) -> Result<(), CatalogError> {
        let (Some(entries), Some(call)) = (&self.entries, self.call) else {
            proof.incomplete();
            return Ok(());
        };
        if call.files.is_empty() {
            proof.incomplete();
        }
        for selection in &call.files {
            if let Some(payload) = selection.projected_payload(entries, SourceUseRole::Selector)? {
                proof.add(payload);
            }
        }
        Ok(())
    }

    fn verified_explicit_patch_absence(
        &self,
        vehicle: &VehicleImplementation,
        directory_complete: bool,
    ) -> bool {
        let Some(call) = self.call.filter(|_| directory_complete) else {
            return false;
        };
        let [LoadoutError::MissingVehicleImplementation { path }] = vehicle.issues() else {
            return false;
        };
        let [attempt] = path.attempts() else {
            return false;
        };
        attempt.kind() == LoadoutDocumentKind::VehiclePatch
            && attempt.source().is_missing_archive()
            && matches!(attempt.read_outcome(), Err(LoadoutFileError::NotFound))
            && call
                .files
                .iter()
                .any(|file| file.matches(attempt, &call.caller))
    }

    fn select_files(
        &self,
        archive: Option<&Archive>,
        proof: &mut Contributions,
    ) -> Result<bool, CatalogError> {
        if self.call.is_some_and(|call| !call.modification_observed) {
            proof.incomplete();
        }
        self.observe_file_choices(archive, proof)
    }

    fn observe_file_choices(
        &self,
        archive: Option<&Archive>,
        proof: &mut Contributions,
    ) -> Result<bool, CatalogError> {
        let (Some(entries), Some(call), Some(archive)) = (&self.entries, self.call, archive) else {
            proof.incomplete();
            return Ok(false);
        };
        entries.check_archive(archive.input_origin())?;
        proof.add(call.caller.selection()?);
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
        Ok(directory_complete)
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
