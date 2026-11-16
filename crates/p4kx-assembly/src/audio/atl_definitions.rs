// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::HashMap;
use std::collections::hash_map::Entry as Slot;
use std::mem;
use std::ptr;

use p4kx_archive::{Archive, Entry, ObservedEntry, ObservedEntryBinding};
use p4kx_extraction::CancelToken;
use p4kx_extraction::sound::wwise_path;

use super::index_read::{self, DefinitionRead};
use super::read;
use super::{AtlTrigger, AudioFileError, AudioIssue, MAX_AUDIO_TRIGGERS};

/// Authored ATL triggers and their Wwise event references from one Archive.
#[derive(Debug, Default)]
pub struct AtlDefinitions {
    files: Vec<DefinitionEntry>,
    triggers: Vec<AtlTrigger>,
    by_name: HashMap<Box<str>, usize>,
    issues: Vec<AudioIssue>,
}

#[derive(Debug)]
struct DefinitionEntry {
    path: Box<str>,
    position: usize,
    owner: Option<ObservedEntryBinding>,
}

impl AtlDefinitions {
    /// Reads only root ATL definition Entries and records Entry failures.
    #[must_use]
    pub fn build(archive: &Archive) -> Self {
        Self::build_until(archive, &CancelToken::new()).unwrap_or_default()
    }

    /// Reads ATL definitions in parallel.
    /// Returns `None` when cancellation is observed between Entries.
    #[must_use]
    pub fn build_until(archive: &Archive, cancel: &CancelToken) -> Option<Self> {
        let mut entries: Vec<_> = archive
            .entries()
            .iter()
            .enumerate()
            .filter(|(_, entry)| wwise_path::is_atl_definition(entry.path()))
            .collect();
        entries.sort_by(|(_, left), (_, right)| read::folded_order(left.path(), right.path()));
        let definitions = index_read::read_definitions(archive, &entries, cancel)?;
        let mut index = Self::default();
        index.add_definitions(archive, &entries, definitions);
        Some(index)
    }

    /// Returns the first authored trigger by folded Entry path, using an ASCII-insensitive name.
    #[must_use]
    pub fn trigger(&self, name: &str) -> Option<&AtlTrigger> {
        self.by_name
            .get(name.to_ascii_lowercase().as_str())
            .and_then(|&position| self.triggers.get(position))
    }

    /// Returns the admitted triggers in definition order.
    #[must_use]
    pub fn triggers(&self) -> &[AtlTrigger] {
        &self.triggers
    }

    /// Returns the original Entry path for a trigger owned by these definitions.
    #[must_use]
    pub fn definition_path(&self, trigger: &AtlTrigger) -> Option<&str> {
        self.definition_file(trigger).map(|(path, _)| path)
    }

    /// Returns the original Archive position for a trigger owned by these definitions.
    #[must_use]
    pub fn definition_entry_position(&self, trigger: &AtlTrigger) -> Option<usize> {
        self.definition_file(trigger).map(|(_, position)| position)
    }

    /// Returns the physically bound Entry behind an admitted trigger.
    #[must_use]
    pub fn definition_source<'archive>(
        &self,
        archive: &'archive Archive,
        trigger: &AtlTrigger,
    ) -> Option<ObservedEntry<'archive>> {
        let file = self.definition_entry(trigger)?;
        let observed = archive.observed_entry(archive.entries().get(file.position)?)?;
        file.owner
            .as_ref()?
            .is_same_entry(&observed.binding())
            .then_some(observed)
    }

    /// Returns the failures encountered while reading and admitting definitions.
    #[must_use]
    pub fn issues(&self) -> &[AudioIssue] {
        &self.issues
    }

    pub(super) fn definition_file(&self, trigger: &AtlTrigger) -> Option<(&str, usize)> {
        let file = self.definition_entry(trigger)?;
        Some((&file.path, file.position))
    }

    fn definition_entry(&self, trigger: &AtlTrigger) -> Option<&DefinitionEntry> {
        if !ptr::eq(self.trigger(trigger.name())?, trigger) {
            return None;
        }
        self.files.get(usize::try_from(trigger.file).ok()?)
    }

    pub(super) fn take_issues(&mut self) -> Vec<AudioIssue> {
        mem::take(&mut self.issues)
    }

    fn add_definitions(
        &mut self,
        archive: &Archive,
        entries: &[(usize, &Entry)],
        definitions: Vec<DefinitionRead>,
    ) {
        self.files = entries
            .iter()
            .map(|&(position, entry)| DefinitionEntry {
                path: entry.path().into(),
                position,
                owner: archive.observed_entry(entry).map(|entry| entry.binding()),
            })
            .collect();
        for ((_, entry), definition) in entries.iter().zip(definitions) {
            let error = match definition {
                Ok(triggers) => self.add_triggers(triggers).err(),
                Err(error) => Some(error),
            };
            if let Some(error) = error {
                self.issues.push(AudioIssue {
                    path: entry.path().into(),
                    error,
                });
            }
        }
    }

    fn add_triggers(&mut self, triggers: Vec<AtlTrigger>) -> Result<(), AudioFileError> {
        for trigger in triggers {
            if let Slot::Vacant(slot) = self.by_name.entry(trigger.name.to_ascii_lowercase().into())
            {
                if self.triggers.len() >= MAX_AUDIO_TRIGGERS {
                    return Err(AudioFileError::TooManyTriggers);
                }
                slot.insert(self.triggers.len());
                self.triggers.push(trigger);
            }
        }
        Ok(())
    }
}
