// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashSet;
use std::io::{self, Read};

use p4kx_assembly::{
    ArchiveModels, RigFile, RigInput, RigInputs, RigParameter, discover_entity_rigs_using,
    discover_model_rig_using,
};
use p4kx_extraction::{DataForge, DataForgeEntry, Entry};

use super::chrparams_selectors::{self, Target};
use super::payload::DefinitionFamily;
use super::scope_health;
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
        if let Some(path) = writer.animation_model.clone() {
            scope.model(&path, lookup, writer)?;
        } else if let Some(selector) = writer.animation_entity.clone() {
            scope.entity(&selector, lookup, writer)?;
        }
        lookup
            .input_state()?
            .definition_scope
            .set(scope)
            .map_err(|_| MediaError::Contract("animation definition scope already planned".into()))
    }

    fn model(
        &mut self,
        path: &str,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let entry = require_target(path, lookup, writer)?;
        if excluded(lookup, entry) {
            return Ok(());
        }
        let mut inputs = Inputs {
            lookup,
            writer,
            targets: Vec::new(),
        };
        let models = ArchiveModels::from_path_index(lookup.index.index(&inputs.work())?);
        let rig = discover_model_rig_using(&models, path, &mut inputs)?;
        scope_health::verify(
            rig.definition.as_ref(),
            rig.skeleton.as_ref(),
            rig.parameters.as_ref(),
        )?;
        self.parameters(rig.parameters.as_ref(), writer)?;
        self.targets(inputs.targets, lookup, writer)
    }

    fn entity(
        &mut self,
        selector: &str,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let archive = lookup.index.archive;
        let entry = DataForgeEntry::locate(archive).ok_or_else(|| {
            MediaError::Contract("selected animation Entity requires a root DataForge".into())
        })?;
        let entry = require_target(entry.path(), lookup, writer)?;
        if excluded(lookup, entry) {
            return Ok(());
        }
        let bytes = read_verified(lookup, entry, writer)?;
        let forge = DataForge::open(&bytes).map_err(|cause| {
            MediaError::Contract(format!("selected animation Entity DataForge: {cause}"))
        })?;
        animation::with_resolved_entity(selector, &forge, lookup, writer, |_, tree, writer| {
            if animation::incomplete_loadout(tree, writer)? {
                return Err(MediaError::Contract(
                    "incomplete animation definition scope: unresolved Entity Loadout".into(),
                ));
            }
            let mut inputs = Inputs {
                lookup,
                writer,
                targets: Vec::new(),
            };
            let models = ArchiveModels::from_path_index(lookup.index.index(&inputs.work())?);
            let rigs = discover_entity_rigs_using(&models, tree, &mut inputs)?;
            for rig in &rigs {
                scope_health::verify(
                    rig.definition.as_ref(),
                    rig.skeleton.as_ref(),
                    rig.parameters.as_ref(),
                )?;
                self.parameters(rig.parameters.as_ref(), writer)?;
            }
            self.targets(inputs.targets, lookup, writer)
        })
    }

    fn parameters(
        &mut self,
        file: Option<&RigFile<Vec<RigParameter>>>,
        writer: &MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        let Some(file) = file else { return Ok(()) };
        let Some(origin) = file.source() else {
            return Ok(());
        };
        let position = origin.entry().index();
        reserve(&mut self.definitions, writer)?;
        self.definitions.insert(position);
        Ok(())
    }

    fn targets(
        &mut self,
        targets: Vec<Target>,
        lookup: &DirectoryLookup<'_>,
        writer: &mut MediaWriter<'_>,
    ) -> Result<(), MediaError> {
        for target in targets {
            let (path, positions) = match target {
                Target::Definition(path) => (path, &mut self.definitions),
                Target::Reference(path) => (path, &mut self.references),
            };
            if let Some(entry) = selected_target(&path, lookup, writer)? {
                reserve(positions, writer)?;
                positions.insert(entry.index());
            }
        }
        Ok(())
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

fn selected_target<'lookup>(
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

fn excluded(lookup: &DirectoryLookup<'_>, entry: &Entry) -> bool {
    lookup.selection.classify(lookup.chain, entry.path()) == EntrySelection::Excluded
}

struct Inputs<'lookup, 'directory, 'writer, 'run> {
    lookup: &'lookup DirectoryLookup<'directory>,
    writer: &'writer MediaWriter<'run>,
    targets: Vec<Target>,
}

impl Inputs<'_, '_, '_, '_> {
    fn work(&self) -> LookupWork<'_> {
        let limits = self.writer.limits();
        LookupWork {
            limits,
            checkpoint: self.writer.checkpoint,
            current: self.lookup.work,
            source_allowance: self.lookup.allowance(limits.sources),
        }
    }
}

impl RigInputs for Inputs<'_, '_, '_, '_> {
    type Error = MediaError;

    fn check(&mut self) -> Result<(), MediaError> {
        self.work().charge(1)
    }

    fn read(&mut self, entry: &Entry) -> Result<RigInput, MediaError> {
        self.check()?;
        if self
            .lookup
            .selection
            .classify(self.lookup.chain, entry.path())
            == EntrySelection::Excluded
        {
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
            let budget = self.lookup.input_state()?.budget;
            let writer = self.writer;
            chrparams_selectors::visit_targets(
                &bytes,
                |count| budget.metadata(0, count, writer.limits(), writer.checkpoint),
                |target| {
                    self.targets
                        .try_reserve(1)
                        .map_err(|cause| allocation(writer, cause))?;
                    self.targets.push(target);
                    Ok(())
                },
            )?;
        }
        Ok(RigInput::Read(bytes))
    }
}

fn read_verified(
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

fn reserve(positions: &mut HashSet<usize>, writer: &MediaWriter<'_>) -> Result<(), MediaError> {
    positions
        .try_reserve(1)
        .map_err(|cause| allocation(writer, cause))
}

fn allocation(writer: &MediaWriter<'_>, cause: std::collections::TryReserveError) -> MediaError {
    error::io(
        &writer.root,
        io::Error::new(io::ErrorKind::OutOfMemory, cause),
    )
}
