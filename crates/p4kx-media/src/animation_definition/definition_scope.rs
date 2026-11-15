// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io::{self, Read};

use p4kx_assembly::{
    ArchiveModels, RigInput, RigInputs, discover_entity_rigs_using, discover_model_rig_using,
};
use p4kx_extraction::{DataForge, DataForgeEntry, Entry};

use super::adb_graph_scope;
use super::dependency_scope::DependencyScope;
use super::entity_database_bindings::Bindings;
use super::payload::DefinitionFamily;
use super::scope_health;
use super::scope_receipt::{self, ScopePlacement};
use crate::animation;
use crate::checkpoint::IO_BLOCK_BYTES;
use crate::lookup_work::LookupWork;
use crate::source_selection::EntrySelection;
use crate::{DirectoryLookup, MediaError, MediaWriter, OriginalIdentifier, error, source};

#[derive(Default)]
pub(crate) struct DefinitionScope {
    definitions: HashSet<usize>,
    references: HashSet<usize>,
}

impl DefinitionScope {
    pub(crate) fn contains_definition(&self, position: usize) -> bool {
        self.definitions.contains(&position)
    }

    pub(crate) fn contains_reference(&self, position: usize) -> bool {
        self.definitions.contains(&position) || self.references.contains(&position)
    }

    pub(super) fn plan(
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        if !writer.scoped_animation_definitions || !lookup.chain.is_empty() {
            return Ok(());
        }
        let mut scope = Self::default();
        let bindings = if let Some(path) = writer.animation_model.clone() {
            scope.model(&path, lookup, writer)?;
            None
        } else if let Some(selector) = writer.animation_entity.clone() {
            scope.entity(&selector, lookup, writer)?
        } else {
            None
        };
        lookup
            .input_state()?
            .definition_scope
            .set(scope)
            .map_err(|_| {
                MediaError::Contract("animation definition scope already planned".into())
            })?;
        if let Some(bindings) = bindings {
            bindings.write(&lookup.for_definition(), writer)?;
        }
        Ok(())
    }

    fn model(
        &mut self,
        path: &str,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let entry = require_target(path, lookup, writer)?;
        if excluded(lookup, entry) {
            return scope_receipt::excluded(lookup, entry, ScopePlacement::Model, writer);
        }
        let mut dependencies = DependencyScope::new(&mut self.definitions, &mut self.references);
        let mut inputs = Inputs {
            lookup,
            writer,
            dependencies: &mut dependencies,
        };
        let models = ArchiveModels::from_path_index(lookup.index.index(&inputs.work()?)?);
        let rig = discover_model_rig_using(&models, path, &mut inputs)?;
        scope_health::verify(
            rig.definition.as_ref(),
            rig.skeleton.as_ref(),
            rig.parameters.as_ref(),
        )?;
        dependencies.expand(lookup, writer)?;
        scope_receipt::model(lookup, entry, &rig, writer)
    }

    fn entity(
        &mut self,
        selector: &str,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<Option<Bindings>, MediaError> {
        let archive = lookup.index.archive;
        let entry = DataForgeEntry::locate(archive).ok_or_else(|| {
            MediaError::Contract("selected animation Entity requires a root DataForge".into())
        })?;
        let entry = require_target(entry.path(), lookup, writer)?;
        if excluded(lookup, entry) {
            scope_receipt::excluded(lookup, entry, ScopePlacement::ExcludedEntityInput, writer)?;
            return Ok(None);
        }
        let bytes = read_verified(lookup, entry, writer)?;
        let forge = DataForge::open(&bytes).map_err(|cause| {
            MediaError::Contract(format!("selected animation Entity DataForge: {cause}"))
        })?;
        animation::with_resolved_entity(
            selector,
            &forge,
            lookup,
            true,
            writer,
            |_, tree, loadout, writer| {
                if let Some(issue) = animation::loadout_issue(tree, writer)? {
                    return Err(MediaError::Contract(format!(
                        "incomplete animation definition scope: unresolved Entity Loadout: {issue}"
                    )));
                }
                let mut dependencies =
                    DependencyScope::new(&mut self.definitions, &mut self.references);
                let mut inputs = Inputs {
                    lookup,
                    writer,
                    dependencies: &mut dependencies,
                };
                let models = ArchiveModels::from_path_index(lookup.index.index(&inputs.work()?)?);
                let rigs = discover_entity_rigs_using(&models, tree, &mut inputs)?;
                for rig in &rigs {
                    scope_health::verify(
                        rig.definition.as_ref(),
                        rig.skeleton.as_ref(),
                        rig.parameters.as_ref(),
                    )?;
                }
                dependencies.expand(lookup, writer)?;
                let bindings = Bindings::collect(&forge, &rigs, loadout, entry, lookup, writer)?;
                adb_graph_scope::expand(
                    bindings.roots(lookup, writer)?,
                    &mut self.definitions,
                    &mut self.references,
                    lookup,
                    writer,
                )?;
                scope_receipt::entity(lookup, entry, &rigs, loadout, writer)?;
                Ok(Some(bindings))
            },
        )
    }
}

fn require_target<'lookup>(
    path: &str,
    lookup: &'lookup DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<&'lookup Entry, MediaError> {
    lookup_target(path, lookup, writer)?.ok_or_else(|| {
        MediaError::Contract(format!(
            "animation scope requires one exact Entry for {path}"
        ))
    })
}

pub(super) fn selected_target<'lookup>(
    path: &str,
    lookup: &'lookup DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<&'lookup Entry>, MediaError> {
    Ok(lookup_target(path, lookup, writer)?.filter(|entry| !excluded(lookup, entry)))
}

fn lookup_target<'lookup>(
    path: &str,
    lookup: &'lookup DirectoryLookup<'_>,
    writer: &mut MediaWriter<'_>,
) -> Result<Option<&'lookup Entry>, MediaError> {
    let observed =
        lookup.lookup_input(OriginalIdentifier::new_with_path(path), path, None, writer)?;
    Ok(observed
        .input
        .and_then(|input| lookup.index.archive.entries().get(input.position)))
}

pub(super) fn excluded(lookup: &DirectoryLookup<'_>, entry: &Entry) -> bool {
    lookup.selection.classify(lookup.chain, entry.path()) == EntrySelection::Excluded
}

struct Inputs<'lookup, 'directory, 'writer, 'run, 'plan, 'scope> {
    lookup: &'lookup DirectoryLookup<'directory>,
    writer: &'writer mut MediaWriter<'run>,
    dependencies: &'plan mut DependencyScope<'scope>,
}

impl Inputs<'_, '_, '_, '_, '_, '_> {
    fn work(&self) -> Result<LookupWork<'_>, MediaError> {
        lookup_work(self.lookup, self.writer)
    }
}

impl RigInputs for Inputs<'_, '_, '_, '_, '_, '_> {
    type Error = MediaError;

    fn check(&mut self) -> Result<(), MediaError> {
        self.work()?.charge(1)
    }

    fn read(&mut self, entry: &Entry) -> Result<RigInput, MediaError> {
        self.check()?;
        if excluded(self.lookup, entry) {
            return Ok(RigInput::NotSelected);
        }
        if entry.is_encrypted() && !self.lookup.index.archive.has_key() {
            return Ok(RigInput::KeyRequired);
        }
        let bytes = read_verified(self.lookup, entry, self.writer)?;
        if matches!(
            DefinitionFamily::of(entry.path()),
            Some(DefinitionFamily::Chrparams)
        ) {
            self.dependencies.observe_parameters(
                entry.index(),
                &bytes,
                self.lookup,
                self.writer,
            )?;
        }
        Ok(RigInput::Read(bytes))
    }
}

pub(super) fn lookup_work<'scope>(
    lookup: &'scope DirectoryLookup<'_>,
    writer: &'scope MediaWriter<'_>,
) -> Result<LookupWork<'scope>, MediaError> {
    let limits = writer.limits();
    Ok(LookupWork {
        limits,
        checkpoint: writer.checkpoint,
        budget: lookup.budget,
        source_allowance: lookup.allowance(limits.sources)?,
    })
}

pub(super) fn read_verified(
    lookup: &DirectoryLookup<'_>,
    entry: &Entry,
    writer: &MediaWriter<'_>,
) -> Result<Vec<u8>, MediaError> {
    let archive = lookup.index.archive;
    let declared = source::entry(lookup.root, archive, entry, lookup.chain)?;
    let result = (|| {
        let limits = writer.limits();
        let size = entry.uncompressed_size();
        if size > limits.entry_bytes {
            return Err(MediaError::Limit {
                budget: "entry_bytes",
                limit: limits.entry_bytes,
            });
        }
        let inputs = lookup.input_state()?;
        inputs.admit(entry.index(), writer)?;
        inputs.budget.bytes(size, limits)?;
        let mut reader = archive
            .open_entry_verified(entry)
            .map_err(|cause| MediaError::Archive(Box::new(cause)))?;
        let capacity = usize::try_from(size)
            .map_err(|cause| error::io(archive.path(), io::Error::other(cause)))?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(capacity)
            .map_err(|cause| allocation(writer, cause))?;
        let mut block = vec![0; IO_BLOCK_BYTES];
        loop {
            writer.checkpoint()?;
            let count = reader
                .read(&mut block)
                .map_err(|cause| error::io(archive.path(), cause))?;
            if count == 0 {
                break;
            }
            if count > capacity.saturating_sub(bytes.len()) {
                return Err(MediaError::Contract(
                    "decoded animation scope input exceeds its Entry size".into(),
                ));
            }
            let chunk = block.get(..count).ok_or_else(|| {
                MediaError::Contract("invalid animation scope reader result".into())
            })?;
            bytes.extend_from_slice(chunk);
        }
        if bytes.len() != capacity {
            return Err(MediaError::Contract(
                "decoded animation scope input disagrees with its Entry size".into(),
            ));
        }
        writer.checkpoint()?;
        Ok(bytes)
    })();
    result.map_err(|cause| error::at(&declared, cause))
}

pub(super) fn allocation(
    writer: &MediaWriter<'_>,
    cause: std::collections::TryReserveError,
) -> MediaError {
    error::io(
        &writer.root,
        io::Error::new(io::ErrorKind::OutOfMemory, cause),
    )
}
