// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use p4kx_archive::{ArchiveInputOrigin, ObservedEntryBinding};

use crate::DecimalU64;
use crate::ProvenanceMode;
use crate::field_origin::{FieldOrigin, OriginAccumulator};
use crate::source_archive_binding::ArchiveSourceBinding;
use crate::source_details::SourceDetails;
use crate::source_file_input::{FileInput, FileInputRef};
use crate::source_inventory::{
    ArchiveDirectorySource, PhysicalSource, PhysicalSourceKind, PhysicalSourceRef,
    SourceEncryptionState,
};
use crate::source_registration;
use crate::source_scope::SourceInput;
use crate::source_use::storage::RetainedLocation;
use crate::source_use::{
    InputScope, LocationView, SourceLocation, SourceUse, SourceUseDescription, SourceUseRef,
    SourceUseRole,
};
use crate::source_use_ref_encoding::{CoordinateEncoding, UseReferences};
use crate::source_value::{SourceValue, SourcedValue};

#[path = "source_registered_uses.rs"]
mod registered_uses;

use self::registered_uses::RegisteredUses;

pub(crate) struct RegisteredSource {
    pub(crate) id: PhysicalSourceRef,
    pub(crate) payload: SourceEncryptionState,
    pub(crate) metadata: SourceEncryptionState,
}

pub(crate) enum InputIdentity {
    Archive {
        origin: ArchiveInputOrigin,
        source_refs: HashMap<ObservedEntryBinding, PhysicalSourceRef>,
    },
    ArchiveView(ArchiveSourceBinding),
    File(FileInput),
}

pub(crate) struct SourceRegistry {
    pub(crate) identity: Arc<()>,
    pub(crate) inputs: Vec<InputIdentity>,
    pub(crate) sources: BTreeMap<PhysicalSourceRef, Arc<PhysicalSource>>,
    pub(crate) uses: RegisteredUses,
    use_references: UseReferences,
    mode: ProvenanceMode,
    sealed: bool,
}

impl SourceRegistry {
    pub(crate) const fn new(identity: Arc<()>) -> Self {
        Self::with_references(identity, UseReferences::Full)
    }

    const fn with_references(identity: Arc<()>, use_references: UseReferences) -> Self {
        Self::with_mode(identity, use_references, ProvenanceMode::Full)
    }

    const fn with_mode(
        identity: Arc<()>,
        use_references: UseReferences,
        mode: ProvenanceMode,
    ) -> Self {
        Self {
            identity,
            inputs: Vec::new(),
            sources: BTreeMap::new(),
            uses: RegisteredUses::new(matches!(
                use_references,
                UseReferences::CompactCoordinates(_)
            )),
            use_references,
            mode,
            sealed: false,
        }
    }

    pub(crate) const fn is_sealed(&self) -> bool {
        self.sealed
    }

    pub(crate) fn seal(&mut self) {
        self.sealed = true;
        self.inputs = Vec::new();
        self.use_references = UseReferences::Full;
    }

    pub(crate) fn with_compact_use_refs(identity: Arc<()>) -> Self {
        Self::with_references(identity, UseReferences::ordinal())
    }

    pub(crate) const fn with_compact_coordinates(identity: Arc<()>) -> Self {
        Self::with_references(
            identity,
            UseReferences::CompactCoordinates(CoordinateEncoding::Standard),
        )
    }

    pub(crate) const fn with_provenance_mode(identity: Arc<()>, mode: ProvenanceMode) -> Self {
        Self::with_mode(
            identity,
            UseReferences::CompactCoordinates(CoordinateEncoding::Standard),
            mode,
        )
    }

    pub(crate) const fn with_field_statistics_coordinates(identity: Arc<()>) -> Self {
        Self::with_references(
            identity,
            UseReferences::CompactCoordinates(CoordinateEncoding::FieldStatistics),
        )
    }

    pub(crate) fn with_batched_use_refs(identity: Arc<()>) -> Self {
        Self::with_references(identity, UseReferences::batched())
    }

    pub(crate) fn take_use_batch(&mut self) -> std::io::Result<RegisteredUses> {
        self.use_references.advance_batch()?;
        Ok(std::mem::take(&mut self.uses))
    }

    pub(crate) fn observe(
        &mut self,
        input: SourceInput<'_>,
        details: SourceDetails,
    ) -> RegisteredSource {
        match input {
            SourceInput::Archive(origin) => self.archive(origin, details),
            SourceInput::ArchiveView(observed) => self.archive_view(observed, details),
            SourceInput::ArchiveFile { input, byte_length } => {
                let index = self.archive_input(input);
                let id = PhysicalSourceRef::new(format!("a{index}:file"));
                let id = if let Some(source) = self.sources.get(&id) {
                    source.id.clone()
                } else {
                    let source = source_registration::archive_file(self, id.clone(), byte_length);
                    self.sources.insert(id.clone(), Arc::new(source));
                    id
                };
                RegisteredSource {
                    id,
                    payload: SourceEncryptionState::Unknown,
                    metadata: SourceEncryptionState::Unknown,
                }
            }

            SourceInput::File(origin) => self.file(FileInputRef::Observed(origin)),
            SourceInput::BuildManifest(origin) => self.file(FileInputRef::BuildManifest(origin)),
            SourceInput::Unknown => {
                let id = PhysicalSourceRef::new(format!("u{}", self.sources.len()));
                self.unknown(id.clone(), Vec::new());
                RegisteredSource {
                    id,
                    payload: SourceEncryptionState::Unknown,
                    metadata: SourceEncryptionState::Unknown,
                }
            }
        }
    }

    pub(crate) fn root_directory(
        &mut self,
        origin: &ArchiveInputOrigin,
        admitted_entry_count: DecimalU64,
    ) -> PhysicalSourceRef {
        let input = self.archive_input(origin);
        let id = PhysicalSourceRef::new(format!("a{input}:directory"));
        self.sources
            .entry(id.clone())
            .or_insert_with(|| {
                Arc::new(PhysicalSource {
                    id,
                    ancestors: Vec::new(),
                    stored_encryption: SourceEncryptionState::Unencrypted,
                    binding: None,
                    source: PhysicalSourceKind::ArchiveDirectory(ArchiveDirectorySource {
                        admitted_entry_count,
                    }),
                })
            })
            .id
            .clone()
    }

    pub(crate) fn archive_input(&mut self, origin: &ArchiveInputOrigin) -> usize {
        if let Some(index) = self.inputs.iter().position(
            |known| matches!(known, InputIdentity::Archive { origin: value, .. } if value.is_same_input(origin)),
        ) {
            return index;
        }
        let index = self.inputs.len();
        self.inputs.push(InputIdentity::Archive {
            origin: origin.clone(),
            source_refs: HashMap::new(),
        });
        index
    }

    fn file(&mut self, origin: FileInputRef<'_>) -> RegisteredSource {
        let index = self.file_input(origin);
        let id = PhysicalSourceRef::new(format!("f{index}"));
        let id = if let Some(source) = self.sources.get(&id) {
            source.id.clone()
        } else {
            let source = source_registration::file(self, id.clone(), origin);
            self.sources.insert(id.clone(), Arc::new(source));
            id
        };
        RegisteredSource {
            id,
            payload: SourceEncryptionState::Unknown,
            metadata: SourceEncryptionState::Unknown,
        }
    }

    fn file_input(&mut self, origin: FileInputRef<'_>) -> usize {
        if let Some(index) = self.inputs.iter().position(
            |known| matches!(known, InputIdentity::File(value) if origin.is_same_input(value)),
        ) {
            return index;
        }
        let index = self.inputs.len();
        self.inputs.push(InputIdentity::File(origin.into_owned()));
        index
    }

    pub(crate) fn unknown(&mut self, id: PhysicalSourceRef, ancestors: Vec<PhysicalSourceRef>) {
        self.sources.entry(id.clone()).or_insert_with(|| {
            Arc::new(PhysicalSource {
                id,
                ancestors,
                stored_encryption: SourceEncryptionState::Unknown,
                binding: None,
                source: PhysicalSourceKind::Unknown,
            })
        });
    }

    pub(crate) fn owns_use(&self, original: &SourceUse) -> bool {
        Arc::ptr_eq(&self.identity, &original.identity)
            && (self.mode == ProvenanceMode::MarkersOnly
                || self
                    .uses
                    .get(original.reference())
                    .is_some_and(|known| Arc::ptr_eq(known, &original.description)))
    }

    pub(crate) fn register_use(
        &mut self,
        source: &PhysicalSourceRef,
        scope: InputScope,
        location: SourceLocation,
        role: SourceUseRole,
        encryption: SourceEncryptionState,
    ) -> SourceUse {
        let (id, full_start) =
            self.use_references
                .reference(source, scope, role, LocationView::Original(&location));
        let compact = matches!(self.use_references, UseReferences::CompactCoordinates(_));
        self.retain_use(id, source, scope, role, encryption, |id| {
            RetainedLocation::new(
                location,
                compact.then_some(id),
                full_start.map(|start| (id, start)),
            )
        })
    }

    pub(crate) fn repeat_use(
        &mut self,
        original: &SourceUseDescription,
        role: SourceUseRole,
    ) -> SourceUse {
        let (id, full_start) = self.use_references.reference(
            original.source(),
            original.scope(),
            role,
            original.location.view(),
        );
        self.retain_use(
            id,
            original.source(),
            original.scope(),
            role,
            original.effective_encryption(),
            |id| original.location.for_reference(id, full_start),
        )
    }

    fn retain_use(
        &mut self,
        id: SourceUseRef,
        source: &PhysicalSourceRef,
        scope: InputScope,
        role: SourceUseRole,
        encryption: SourceEncryptionState,
        location: impl FnOnce(&SourceUseRef) -> RetainedLocation,
    ) -> SourceUse {
        let describe = |id| SourceUseDescription {
            location: location(&id),
            id,
            source: source.clone(),
            scope,
            role,
            effective_encryption: encryption,
        };
        let description = if self.mode == ProvenanceMode::Full || role == SourceUseRole::Attempted {
            self.uses.get_or_insert_with(id, describe)
        } else {
            Arc::new(describe(id))
        };
        SourceUse {
            description,
            identity: Arc::clone(&self.identity),
        }
    }

    pub(crate) fn origin_at(
        &mut self,
        source: &PhysicalSourceRef,
        scope: InputScope,
        location: SourceLocation,
        role: SourceUseRole,
        encryption: SourceEncryptionState,
        complete: bool,
    ) -> FieldOrigin {
        if self.mode == ProvenanceMode::MarkersOnly && role != SourceUseRole::Attempted {
            let mut origin = OriginAccumulator::new(complete, self.mode);
            if role.contributes() {
                origin.include_source(
                    encryption,
                    matches!(location, SourceLocation::Opaque { .. }),
                );
            }
            return origin.finish(Arc::clone(&self.identity));
        }
        let used = self.register_use(source, scope, location, role, encryption);
        FieldOrigin::new(Arc::clone(&self.identity), [&used], complete, self.mode)
    }

    pub(crate) fn file_metadata_value<T>(
        &mut self,
        source: &PhysicalSourceRef,
        field: String,
        value: SourceValue<T>,
    ) -> SourcedValue<T> {
        let origin = self.origin_at(
            source,
            InputScope::FileMetadata,
            SourceLocation::Metadata { field },
            SourceUseRole::Payload,
            SourceEncryptionState::Unknown,
            true,
        );
        SourcedValue::new(value, origin)
    }

    pub(crate) fn metadata_value<T>(
        &mut self,
        source: &PhysicalSourceRef,
        field: String,
        value: SourceValue<T>,
        encryption: SourceEncryptionState,
    ) -> SourcedValue<T> {
        let origin = self.origin_at(
            source,
            InputScope::ArchiveDirectoryMetadata,
            SourceLocation::Metadata { field },
            SourceUseRole::Payload,
            encryption,
            true,
        );
        SourcedValue::new(value, origin)
    }
}

#[cfg(test)]
#[path = "source_registry_value_tests.rs"]
mod value_tests;
