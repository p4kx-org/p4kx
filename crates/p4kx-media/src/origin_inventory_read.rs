// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use p4kx_extraction::FieldOrigin;

use crate::checkpoint::Checkpoint;
use crate::origin_contract::OriginReceipts;
use crate::origin_inventory_wire::{
    DirectoryOrdinal, Encryption, LocationKind, Role, Scope, SourceKind, SourceRow, UseRow,
};
use crate::origin_line_reader::OriginLines;
use crate::origin_read_policy::{InventoryBudget, OriginReadPolicy};
use crate::origin_rows_read::ReadOrigin;
use crate::origin_source_facts::SourceFacts;
use crate::{MediaError, origin_inventory_wire};

#[path = "origin_use_sets_read.rs"]
mod use_sets;
use self::use_sets::{Evidence, ReadSet};

/// Descriptive wire facts only, never original-input or `SourceUse` authority.
pub(crate) struct ReadInventory {
    sources: BTreeMap<String, SourceFacts>,
    facts: Vec<FieldOrigin>,
    uses: BTreeMap<String, UseFacts>,
    sets: Vec<ReadSet>,
}

struct UseFacts {
    source: String,
    scope: Scope,
    role: Role,
    location: UseLocation,
    effective_encryption: Encryption,
}
struct UseLocation {
    kind: LocationKind,
    selector: Option<String>,
    directory_paths: Option<[DirectoryOrdinal; 2]>,
    installation_flags: Option<bool>,
}

impl ReadInventory {
    pub(crate) fn open(
        root: &Path,
        receipts: &OriginReceipts,
        checkpoint: Checkpoint<'_>,
        policy: OriginReadPolicy,
    ) -> Result<Self, MediaError> {
        let mut inventory = Self {
            sources: BTreeMap::new(),
            facts: Vec::new(),
            uses: BTreeMap::new(),
            sets: Vec::new(),
        };
        let mut budget = InventoryBudget::new(policy);
        inventory.read_sources(root, receipts, checkpoint, policy, &mut budget)?;
        inventory.read_uses(root, receipts, checkpoint, policy, &mut budget)?;
        inventory.validate(&mut budget)?;
        let sets = OriginLines::open(root, &receipts.sets, checkpoint, policy.line_bytes)?;
        inventory.sets = use_sets::read(sets, &mut budget, |id| inventory.contributor(id))?;
        Ok(inventory)
    }

    fn read_sources(
        &mut self,
        root: &Path,
        receipts: &OriginReceipts,
        checkpoint: Checkpoint<'_>,
        policy: OriginReadPolicy,
        budget: &mut InventoryBudget,
    ) -> Result<(), MediaError> {
        let mut lines = OriginLines::open(root, &receipts.sources, checkpoint, policy.line_bytes)?;
        while let Some(source) = lines.next(origin_inventory_wire::seed::<SourceRow>())? {
            require_reference(&source.id)?;
            budget.charge(source.id.len())?;
            for ancestor in &source.ancestors {
                require_reference(ancestor)?;
                budget.charge(ancestor.len())?;
            }
            let (reference, source) =
                SourceFacts::capture(source, policy, budget, &mut self.facts)?;
            if self.sources.insert(reference, source).is_some() {
                return Err(invalid("duplicate physical source reference"));
            }
        }
        lines.finish()
    }

    fn read_uses(
        &mut self,
        root: &Path,
        receipts: &OriginReceipts,
        checkpoint: Checkpoint<'_>,
        policy: OriginReadPolicy,
        budget: &mut InventoryBudget,
    ) -> Result<(), MediaError> {
        let mut lines = OriginLines::open(root, &receipts.uses, checkpoint, policy.line_bytes)?;
        while let Some(mut used) = lines.next(origin_inventory_wire::seed::<UseRow>())? {
            used.location.require_shape()?;
            require_reference(&used.id)?;
            require_reference(&used.source)?;
            budget.charge(used.id.len())?;
            budget.charge(used.source.len())?;
            if let Some(selector) = &used.location.selector {
                require_reference(selector)?;
                budget.charge(selector.len())?;
            }
            let reference = std::mem::take(&mut used.id);
            let facts = UseFacts {
                source: used.source,
                scope: used.scope,
                role: used.role,
                location: UseLocation {
                    kind: used.location.kind,
                    directory_paths: used.location.directory_paths(),
                    installation_flags: used.location.installation_flags(),
                    selector: used.location.selector,
                },
                effective_encryption: used.effective_encryption,
            };
            if self.uses.insert(reference, facts).is_some() {
                return Err(invalid("duplicate source-use reference"));
            }
        }
        lines.finish()
    }

    fn validate(&self, budget: &mut InventoryBudget) -> Result<(), MediaError> {
        self.validate_sources()?;
        let states = self.ancestor_states(budget)?;
        self.validate_uses(&states)?;
        for fact in &self.facts {
            self.validate_origin(fact)?;
        }
        Ok(())
    }
    fn validate_sources(&self) -> Result<(), MediaError> {
        for (reference, source) in &self.sources {
            if (matches!(source.kind, SourceKind::ArchiveFile | SourceKind::File)
                && (!source.ancestors.is_empty()
                    || source.stored_encryption != Encryption::Unknown))
                || (source.kind == SourceKind::ArchiveDirectory
                    && (!source.ancestors.is_empty()
                        || source.stored_encryption != Encryption::Unencrypted))
                || (source.kind == SourceKind::Unknown
                    && source.stored_encryption != Encryption::Unknown)
            {
                return Err(invalid(
                    "physical source kind contradicts its stored evidence",
                ));
            }
            let mut seen = BTreeSet::new();
            for ancestor in &source.ancestors {
                if ancestor == reference
                    || self
                        .sources
                        .get(ancestor)
                        .is_none_or(|source| source.kind == SourceKind::ArchiveDirectory)
                    || !seen.insert(ancestor)
                {
                    return Err(invalid("invalid physical source ancestor closure"));
                }
            }
        }
        Ok(())
    }

    fn validate_directory_location(
        used: &UseFacts,
        source: &SourceFacts,
    ) -> Result<(), MediaError> {
        if let Some(single_byte) = used.location.installation_flags {
            if single_byte
                && source.kind == SourceKind::ArchiveDirectory
                && used.scope == Scope::ArchiveDirectoryMetadata
                && used.role == Role::Payload
            {
                return Ok(());
            }
            return Err(invalid(
                "installation flags contradict their original directory byte",
            ));
        }
        match (source.kind, used.location.directory_paths) {
            (SourceKind::ArchiveDirectory, Some([start, end]))
                if used.role == Role::Selector
                    && source.admitted_entry_count.is_some_and(|count| {
                        match used.location.kind {
                            LocationKind::ArchiveDirectoryPaths => start.get() == 0 && end == count,
                            LocationKind::ArchiveDirectoryTreePaths => {
                                start.get() <= end.get() && end.get() <= count.get()
                            }
                            _ => false,
                        }
                    }) =>
            {
                Ok(())
            }
            (SourceKind::ArchiveDirectory, _) | (_, Some(_)) => Err(invalid(
                "directory path survey contradicts its original owner",
            )),
            _ => Ok(()),
        }
    }

    fn validate_uses(&self, states: &BTreeMap<&str, Encryption>) -> Result<(), MediaError> {
        for used in self.uses.values() {
            let source = self
                .sources
                .get(&used.source)
                .ok_or_else(|| invalid("source use refers to a missing physical source"))?;
            Self::validate_use_encryption(used, source, states)?;
            Self::validate_directory_location(used, source)?;
            if used.location.kind == LocationKind::LocalizationLookup {
                let Some(selector) = &used.location.selector else {
                    return Err(invalid("localization use lacks its selector reference"));
                };
                if !self.uses.contains_key(selector) {
                    return Err(invalid("localization use selector reference is missing"));
                }
            }
        }
        Ok(())
    }

    fn validate_use_encryption(
        used: &UseFacts,
        source: &SourceFacts,
        states: &BTreeMap<&str, Encryption>,
    ) -> Result<(), MediaError> {
        let mut ancestors = Encryption::Unencrypted;
        for reference in &source.ancestors {
            let ancestor = states
                .get(reference.as_str())
                .ok_or_else(|| invalid("physical source ancestor is missing"))?;
            ancestors = ancestors.combine(*ancestor);
        }
        if !source.accepts_encryption(used.scope, ancestors, used.effective_encryption) {
            return Err(invalid(
                "captured source-use encryption contradicts physical source evidence",
            ));
        }
        Ok(())
    }

    fn ancestor_states(
        &self,
        budget: &mut InventoryBudget,
    ) -> Result<BTreeMap<&str, Encryption>, MediaError> {
        let mut done = BTreeMap::new();
        let mut active = BTreeSet::new();
        for (reference, source) in &self.sources {
            if done.contains_key(reference.as_str()) {
                continue;
            }
            let mut stack = vec![(reference.as_str(), 0_usize, source.stored_encryption)];
            active.insert(reference.as_str());
            while let Some((reference, next, state)) = stack.last_mut() {
                let source = self
                    .sources
                    .get(*reference)
                    .ok_or_else(|| invalid("physical ancestor is missing"))?;
                if let Some(ancestor) = source.ancestors.get(*next) {
                    *next += 1;
                    if let Some(ancestor_state) = done.get(ancestor.as_str()) {
                        *state = state.combine(*ancestor_state);
                    } else {
                        if !active.insert(ancestor.as_str()) {
                            return Err(invalid("physical source ancestors form a cycle"));
                        }
                        let source = self
                            .sources
                            .get(ancestor)
                            .ok_or_else(|| invalid("physical ancestor is missing"))?;
                        stack.push((ancestor.as_str(), 0, source.stored_encryption));
                    }
                } else {
                    let (reference, _, state) = stack
                        .pop()
                        .ok_or_else(|| invalid("physical ancestor traversal is empty"))?;
                    active.remove(reference);
                    budget.charge(size_of::<Encryption>())?;
                    done.insert(reference, state);
                    if let Some((_, _, parent)) = stack.last_mut() {
                        *parent = parent.combine(state);
                    }
                }
            }
        }
        Ok(done)
    }
    pub(crate) fn validate_origin(&self, origin: &FieldOrigin) -> Result<(), MediaError> {
        let mut evidence = Evidence::empty();
        for reference in origin.source_uses_refs() {
            evidence.include(self.contributor(reference.as_str())?);
        }
        evidence.validate(
            origin.contains_encrypted_source(),
            origin.source_set_complete(),
        )
    }

    pub(crate) fn validate_field(
        &self,
        origin: &ReadOrigin,
        limit: usize,
    ) -> Result<(), MediaError> {
        let Some(set) = origin.set else {
            return self.validate_origin(&origin.local);
        };
        let index = usize::try_from(set.0.get())
            .map_err(|_| invalid("field origin use-set reference is not representable"))?;
        let set = self
            .sets
            .get(index)
            .ok_or_else(|| invalid("field origin use-set reference is missing"))?;
        let local = origin.local.source_uses_refs();
        if set.members.len().saturating_add(local.len()) > limit {
            return Err(invalid("field origin use set exceeds reader policy"));
        }
        let mut evidence = set.evidence;
        for reference in local {
            if set.contains(reference.as_str()) {
                return Err(invalid("field-local source use repeats a shared member"));
            }
            evidence.include(self.contributor(reference.as_str())?);
        }
        evidence.validate(
            origin.contains_encrypted_source(),
            origin.source_set_complete(),
        )
    }

    fn contributor(&self, reference: &str) -> Result<Evidence, MediaError> {
        let used = self
            .uses
            .get(reference)
            .ok_or_else(|| invalid("field origin source-use reference is missing"))?;
        if !used.role.contributes() {
            return Err(invalid(
                "field origin includes an attempted or untouched reference",
            ));
        }
        Ok(Evidence {
            encryption: used.effective_encryption,
            opaque: used.location.kind == LocationKind::Opaque,
        })
    }
}

fn require_reference(reference: &str) -> Result<(), MediaError> {
    if reference.is_empty() {
        return Err(invalid("document-local origin reference is empty"));
    }
    Ok(())
}

fn invalid(message: &str) -> MediaError {
    MediaError::Destination(message.to_owned())
}

#[cfg(test)]
#[path = "origin_directory_survey_tests.rs"]
mod directory_survey_tests;
