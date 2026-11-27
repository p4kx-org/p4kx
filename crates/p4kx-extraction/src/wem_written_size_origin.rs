// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use p4kx_archive::{Archive, ObservedEntryBinding};

use crate::{
    FieldOrigin, InputScope, SourceEvidenceError, SourceInput, SourceInventorySelection,
    SourceLocation, SourceUseRole, SourceUses,
};

/// Original inputs of the measured byte length of a committed WEM, WAV or Ogg file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WemWrittenSizeOrigin {
    entry: ObservedEntryBinding,
    inputs: Vec<SizeInput>,
    complete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SizeInput {
    scope: InputScope,
    location: SourceLocation,
    role: SourceUseRole,
}

impl WemWrittenSizeOrigin {
    pub(super) fn capture(
        entry: ObservedEntryBinding,
        selection: &SourceInventorySelection<'_>,
        complete: bool,
    ) -> Option<Self> {
        let source = selection.iter_sources().find(|source| {
            source
                .binding
                .as_ref()
                .is_some_and(|binding| binding.matches(&entry))
        })?;
        let mut inputs = Vec::with_capacity(selection.iter_uses().len());
        for input in selection.iter_uses() {
            if input.source() != source.id()
                || matches!(input.location(), SourceLocation::LocalizationLookup { .. })
            {
                return None;
            }
            inputs.push(SizeInput {
                scope: input.scope(),
                location: input.location().clone(),
                role: input.role(),
            });
        }
        Some(Self {
            entry,
            inputs,
            complete,
        })
    }

    /// Rebinds the recorded size contributors to the caller's inventory for the original Archive.
    ///
    /// A missing original Entry binding yields an unknown origin.
    ///
    /// # Errors
    ///
    /// Returns an error when the caller's source registry cannot record the original uses.
    pub fn bind(
        &self,
        archive: &Archive,
        uses: &SourceUses,
    ) -> Result<FieldOrigin, SourceEvidenceError> {
        let observed = archive
            .entries()
            .get(self.entry.entry_index() as usize)
            .and_then(|entry| archive.observed_entry(entry))
            .filter(|entry| self.entry.is_same_entry(&entry.binding()));
        let Some(observed) = observed else {
            return Ok(uses.origin(&[], false));
        };
        let scope = uses.observe(SourceInput::ArchiveView(observed), InputScope::Payload)?;
        let inputs = self
            .inputs
            .iter()
            .map(|input| {
                scope
                    .for_scope(input.scope)
                    .at(input.location.clone(), input.role)
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(uses.origin(&inputs, self.complete))
    }
}
