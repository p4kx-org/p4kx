// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_assembly::{LoadoutDocumentKind, LoadoutFileObservation, VehicleXmlSite};
use p4kx_extraction::{InputScope, SourceInput, SourceLocation, SourceUseRole, SourceValue};

use super::{AuthoredEntryPath, FileGraph};
use crate::build::original_input::OriginalCatalogInput;
use crate::catalog_read::original::{EntryCapture, OriginalDatum, RecordSubject};
use crate::{CatalogError, ExportAccountingError};

/// Original recipes belonging to one current physical vehicle caller.
#[derive(Debug, Clone)]
pub(crate) struct VehicleCaller {
    subject: RecordSubject,
    definition: AuthoredEntryPath,
    modification: Option<AuthoredEntryPath>,
    class_name: AuthoredEntryPath,
}

impl VehicleCaller {
    pub(crate) fn read(
        input: &OriginalCatalogInput<'_, '_>,
        subject: &RecordSubject,
        definition: AuthoredEntryPath,
        modification: Option<AuthoredEntryPath>,
        class_name: AuthoredEntryPath,
    ) -> Result<Self, CatalogError> {
        let caller = Self {
            subject: subject.clone(),
            definition,
            modification,
            class_name,
        };
        caller.check_input(input)?;
        Ok(caller)
    }

    pub(crate) fn record_index(&self) -> Result<u32, ExportAccountingError> {
        self.subject
            .key()
            .map(crate::catalog_read::original::RecordSubjectKey::index)
            .ok_or(ExportAccountingError::UnboundHealthSubject)
    }

    fn paths(&self) -> impl Iterator<Item = &AuthoredEntryPath> {
        std::iter::once(&self.definition)
            .chain(self.modification.iter())
            .chain(std::iter::once(&self.class_name))
    }

    pub(crate) fn check_input(
        &self,
        input: &OriginalCatalogInput<'_, '_>,
    ) -> Result<(), ExportAccountingError> {
        self.subject.check_input(input)?;
        let index = self.record_index()?;
        for path in self.paths() {
            path.check_input(input)?;
            if path.record_owner()? != index {
                return Err(ExportAccountingError::ConflictingSourceDatum);
            }
        }
        Ok(())
    }

    pub(crate) fn selection(&self) -> Result<OriginalDatum<()>, CatalogError> {
        let mut inputs = Vec::new();
        let mut complete = true;
        for path in self.paths() {
            let (selected, known) = path.selector_inputs()?;
            inputs.extend(selected);
            complete &= known;
        }
        Ok(OriginalDatum::observed(
            SourceValue::Present(()),
            inputs,
            complete,
        ))
    }

    pub(crate) fn is_same_call(&self, other: &Self) -> bool {
        self.subject.key() == other.subject.key()
            && self.definition.is_same_recipe(&other.definition)
            && self.class_name.is_same_recipe(&other.class_name)
            && match (&self.modification, &other.modification) {
                (Some(left), Some(right)) => left.is_same_recipe(right),
                (None, None) => true,
                _ => false,
            }
    }
}

/// An actual file event with the current caller rather than the first cached caller.
#[derive(Debug, Clone)]
pub(crate) struct VehicleFileSelection {
    original: LoadoutFileObservation,
    caller: VehicleCaller,
}

impl FileGraph {
    pub(crate) fn vehicle_selection(
        &self,
        original: &LoadoutFileObservation,
        caller: VehicleCaller,
    ) -> Result<VehicleFileSelection, CatalogError> {
        let state = crate::export_evidence_lock::lock(&self.state)?;
        state.open()?;
        if state.position(original).is_none()
            || !matches!(
                original.kind(),
                LoadoutDocumentKind::VehicleImplementation | LoadoutDocumentKind::VehiclePatch
            )
        {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        drop(state);
        Ok(VehicleFileSelection {
            original: original.clone(),
            caller,
        })
    }
}

impl VehicleFileSelection {
    pub(crate) const fn caller(&self) -> &VehicleCaller {
        &self.caller
    }

    pub(crate) fn needs_directory(&self) -> bool {
        self.original.source().is_missing_archive()
    }

    pub(crate) fn matches(
        &self,
        original: &LoadoutFileObservation,
        caller: &VehicleCaller,
    ) -> bool {
        self.original.is_same_operation(original) && self.caller.is_same_call(caller)
    }

    pub(crate) fn site(
        &self,
        capture: &EntryCapture<'_>,
        site: &VehicleXmlSite,
        role: SourceUseRole,
    ) -> Result<OriginalDatum<()>, CatalogError> {
        if !self.original.is_same_operation(site.file())
            || self.original.read_outcome().is_err()
            || !matches!(self.original.projection_outcome(), Some(Ok(())))
        {
            return Err(ExportAccountingError::UnobservedFailure.into());
        }
        let origin = self
            .original
            .source()
            .entry_origin()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        capture.check_origin(origin)?;
        let sources = capture.source_uses();
        let mut inputs = Vec::new();
        let complete = site.location().is_some();
        inputs.push(
            sources
                .observe(
                    SourceInput::Archive(origin),
                    InputScope::ArchiveDirectoryMetadata,
                )?
                .at(
                    SourceLocation::Metadata {
                        field: "path".to_owned(),
                    },
                    SourceUseRole::Selector,
                )?,
        );
        let payload = sources.observe(SourceInput::Archive(origin), InputScope::Payload)?;
        inputs.push(match site.location() {
            Some(location) => payload.at(location.clone(), role)?,
            None => {
                payload.at_opaque(role, p4kx_extraction::SourceCoordinateOmission::Unavailable)?
            }
        });
        Ok(OriginalDatum::observed(
            SourceValue::Present(()),
            inputs,
            complete,
        ))
    }
    pub(crate) fn attempt(
        &self,
        capture: &EntryCapture<'_>,
        directory: Option<&p4kx_extraction::SourceUse>,
        directory_complete: bool,
    ) -> Result<OriginalDatum<()>, CatalogError> {
        let source = self.original.source();
        if let Some(input) = source.archive_input_origin() {
            capture.check_archive(input)?;
        }
        let uses = capture.source_uses();
        let mut inputs = Vec::new();
        let complete = if let Some(origin) = source.entry_origin() {
            capture.check_origin(origin)?;
            inputs.push(
                uses.observe(
                    SourceInput::Archive(origin),
                    InputScope::ArchiveDirectoryMetadata,
                )?
                .at(
                    SourceLocation::Metadata {
                        field: "path".to_owned(),
                    },
                    SourceUseRole::Selector,
                )?,
            );
            let good = self.original.read_outcome().is_ok()
                && matches!(self.original.projection_outcome(), Some(Ok(())));
            if !good {
                inputs.push(
                    uses.observe(SourceInput::Archive(origin), InputScope::Payload)?
                        .at(
                            SourceLocation::Subresource {
                                namespace: "loadoutFileRead".to_owned(),
                                indices: Vec::new(),
                                field: "content".to_owned(),
                            },
                            SourceUseRole::Attempted,
                        )?,
                );
            }
            good
        } else if source.is_missing_archive()
            && matches!(
                self.original.read_outcome(),
                Err(p4kx_assembly::LoadoutFileError::NotFound)
            )
        {
            inputs.extend(directory.cloned());
            directory.is_some() && directory_complete
        } else {
            return Err(ExportAccountingError::UnboundHealthSubject.into());
        };
        Ok(OriginalDatum::observed(
            SourceValue::Present(()),
            inputs,
            complete,
        ))
    }
}
