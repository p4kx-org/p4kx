// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_export::Language;
use p4kx_extraction::{
    FieldOrigin, InputScope, PhysicalSource, PhysicalSourceKind, SourceEncryptionState,
    SourceInventory, SourceLocation, SourceUseDescription, SourceUseRole,
};

use super::TestResult;

#[derive(Default)]
pub(super) struct Closure<'query> {
    origins: Vec<&'query FieldOrigin>,
    pub(super) owners: Vec<(u32, &'query FieldOrigin)>,
    pub(super) lookups: Vec<(Language, u32, &'query FieldOrigin)>,
}

type Uses<'query> = HashMap<&'query str, &'query SourceUseDescription>;
type Sources<'query> = HashMap<&'query str, &'query PhysicalSource>;

impl<'query> Closure<'query> {
    pub(super) fn retain(&mut self, origin: &'query FieldOrigin) {
        self.origins.push(origin);
    }

    pub(super) fn verify(self, inventory: &SourceInventory, dataforge: u32) -> TestResult<usize> {
        let needed: HashSet<&str> = self
            .origins
            .iter()
            .flat_map(|origin| origin.source_uses_refs())
            .map(p4kx_extraction::SourceUseRef::as_str)
            .collect();
        let uses: Uses<'_> = inventory
            .iter_uses()
            .filter(|row| needed.contains(row.id().as_str()))
            .map(|row| (row.id().as_str(), row))
            .collect();
        let sources: Sources<'_> = inventory
            .iter_sources()
            .map(|row| (row.id().as_str(), row))
            .collect();
        if needed.is_empty() || uses.len() != needed.len() {
            return Err("Ship text provenance does not resolve to the selected inventory".into());
        }
        verify_use_sources(&uses, &sources, dataforge)?;
        for origin in self.origins {
            verify_marker(origin, &uses)?;
        }
        for (index, origin) in self.owners {
            if !original_record(index, origin, &uses, &sources, dataforge) {
                return Err("a Ship text cell lacks its own original Record".into());
            }
        }
        for (language, ordinal, origin) in self.lookups {
            verify_lookup(language, ordinal, origin, &uses, &sources)?;
        }
        Ok(uses.len())
    }
}

fn verify_use_sources(uses: &Uses<'_>, sources: &Sources<'_>, dataforge: u32) -> TestResult {
    for observation in uses.values() {
        let source = sources
            .get(observation.source().as_str())
            .ok_or("missing physical Ship text source")?;
        if !matches!(
            observation.role(),
            SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
        ) || source
            .ancestors()
            .iter()
            .any(|ancestor| !sources.contains_key(ancestor.as_str()))
        {
            return Err("a Ship text datum lacks contributing physical evidence".into());
        }
        if let SourceLocation::LocalizationLookup { selector, .. } = observation.location() {
            let authored = uses
                .get(selector.as_str())
                .ok_or("missing original lookup selector")?;
            if authored.role() != SourceUseRole::Selector || !is_entry(sources, authored, dataforge)
            {
                return Err("a Ship locale lookup lacks its original DataForge selector".into());
            }
        }
    }
    Ok(())
}

fn verify_marker(origin: &FieldOrigin, uses: &Uses<'_>) -> TestResult {
    let references = origin.source_uses_refs();
    if !origin.source_set_complete()
        || references.is_empty()
        || references
            .array_windows::<2>()
            .any(|[earlier, later]| earlier >= later)
    {
        return Err("incomplete or unordered direct Ship text provenance".into());
    }
    let mut plain = true;
    let mut encrypted = false;
    for reference in references {
        let state = uses
            .get(reference.as_str())
            .ok_or("unresolved Ship text use")?
            .effective_encryption();
        plain &= state == SourceEncryptionState::Unencrypted;
        encrypted |= state == SourceEncryptionState::Encrypted;
    }
    let expected = if encrypted {
        Some(true)
    } else if plain {
        Some(false)
    } else {
        None
    };
    if origin.contains_encrypted_source() != expected {
        return Err("Ship text encryption disagrees with its actual contributing uses".into());
    }
    Ok(())
}

fn original_record(
    index: u32,
    origin: &FieldOrigin,
    uses: &Uses<'_>,
    sources: &Sources<'_>,
    dataforge: u32,
) -> bool {
    origin.source_uses_refs().iter().any(|reference| {
        let Some(observation) = uses.get(reference.as_str()) else {
            return false;
        };
        is_entry(sources, observation, dataforge)
            && match observation.location() {
                SourceLocation::DataForge { record_index, .. } => *record_index == index,
                SourceLocation::Subresource {
                    namespace, indices, ..
                } => {
                    namespace.starts_with("dataforge")
                        && indices.first().copied() == Some(u64::from(index))
                }
                _ => false,
            }
    })
}

fn verify_lookup(
    language: Language,
    ordinal: u32,
    origin: &FieldOrigin,
    uses: &Uses<'_>,
    sources: &Sources<'_>,
) -> TestResult {
    let mut found = false;
    for reference in origin.source_uses_refs() {
        let observation = uses
            .get(reference.as_str())
            .ok_or("missing Ship translation use")?;
        if let SourceLocation::LocalizationLookup {
            language: observed, ..
        } = observation.location()
        {
            if *observed != language || !is_entry(sources, observation, ordinal) {
                return Err("a Ship translation depends on the wrong original locale Entry".into());
            }
            found |= observation.role() == SourceUseRole::Selector;
        }
    }
    if !found {
        return Err("a Ship translation lacks its verified original locale lookup".into());
    }
    Ok(())
}

fn is_entry(sources: &Sources<'_>, observation: &SourceUseDescription, ordinal: u32) -> bool {
    let Some(source) = sources.get(observation.source().as_str()) else {
        return false;
    };
    if observation.scope() != InputScope::Payload || !source.ancestors().is_empty() {
        return false;
    }
    match source.kind() {
        PhysicalSourceKind::ArchiveEntry(entry) => {
            u32::try_from(entry.entry_index()) == Ok(ordinal)
        }
        PhysicalSourceKind::ArchiveEntryProof(entry) => {
            entry.entry_index().get() == u64::from(ordinal)
        }
        _ => false,
    }
}
