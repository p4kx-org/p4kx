// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::BTreeMap;

use p4kx_export::catalog_queries::{ContractOccurrence, ContractSnapshot, SourceValueRef};
use p4kx_extraction::{
    DataForgeEntry, DataForgeOrigin, FieldOrigin, InputScope, RecordTree, SourceEncryption,
    SourceInput, SourceLocation, SourceScope, SourceUseDescription, SourceUseRef, SourceUseRole,
    SourceUses, Value,
};

use super::TestResult;
use super::source::Rows;
use crate::real_archive_auto_spawns::original::Locations;

pub(super) struct Evidence<'read, 'source> {
    pub(super) scope: SourceScope,
    pub(super) locations: Locations<'source>,
    uses: BTreeMap<SourceUseRef, &'read SourceUseDescription>,
    owner: u32,
    encrypted: Option<bool>,
    sites: BTreeMap<Slots, usize>,
}

impl<'read, 'source> Evidence<'read, 'source> {
    pub(super) fn read(
        entry: &DataForgeEntry,
        tree: &RecordTree<'source>,
        snapshot: &'read ContractSnapshot<'_>,
        sources: &Rows<'_, 'source>,
    ) -> TestResult<Self> {
        let DataForgeOrigin::Archive(original) = entry.origin() else {
            return Err("original DataForge must belong to the Archive".into());
        };
        let inventory = SourceUses::new();
        let scope = inventory.observe(SourceInput::Archive(original), InputScope::Payload)?;
        let observed = inventory.freeze()?;
        if observed.iter_sources().any(|physical| {
            !snapshot
                .source_inventory
                .iter_sources()
                .any(|actual| actual == physical)
        }) {
            return Err("SDK inventory differs from the independently observed original Entry or its ancestors".into());
        }
        let locations = Locations::read(tree.root());
        let sites = original_sites(sources, &locations)?;
        Ok(Self {
            scope,
            locations,
            sites,
            owner: tree.record_index(),
            uses: snapshot
                .source_inventory
                .iter_uses()
                .map(|used| (used.id().clone(), used))
                .collect(),
            encrypted: match entry.origin().encryption() {
                SourceEncryption::Encrypted => Some(true),
                SourceEncryption::Unencrypted => Some(false),
                _ => None,
            },
        })
    }

    pub(super) fn site(&self, occurrence: &ContractOccurrence) -> TestResult<usize> {
        let slots = Slots {
            handler: occurrence.handler().value_index(),
            contract: occurrence.contract().value_index(),
            sub: occurrence.sub_contract().map(SourceValueRef::value_index),
        };
        self.sites.get(&slots).copied().ok_or_else(|| "SDK occurrence does not identify any original handler, contract and sub-contract slots".into())
    }

    pub(super) fn operand(&self, original: &Value<'source>, origin: &FieldOrigin) -> TestResult {
        if !origin.source_set_complete() || origin.contains_encrypted_source() != self.encrypted {
            return Err(
                "identity source completeness or encryption differs from the original Entry".into(),
            );
        }
        let ordinal = u64::try_from(self.locations.ordinal(original)?)?;
        let mut actual = false;
        for reference in origin.source_uses_refs() {
            let used = self
                .uses
                .get(reference)
                .ok_or("identity has a dangling source-use reference")?;
            if used.source() != self.scope.source_ref()
                || used.scope() != InputScope::Payload
                || !matches!(
                    used.role(),
                    SourceUseRole::Payload | SourceUseRole::Selector | SourceUseRole::Conversion
                )
            {
                return Err("identity cites a foreign or noncontributing source".into());
            }
            actual |= matches!(used.location(), SourceLocation::Subresource { namespace, indices, field }
                if namespace == "dataforgeValue" && indices.as_slice() == [u64::from(self.owner), ordinal] && field == "$value");
        }
        if !actual {
            return Err("identity lost its exact original physical value occurrence".into());
        }
        Ok(())
    }
}

#[derive(PartialEq, Eq, PartialOrd, Ord)]
struct Slots {
    handler: u64,
    contract: u64,
    sub: Option<u64>,
}

fn original_sites<'source>(
    sources: &Rows<'_, 'source>,
    locations: &Locations<'source>,
) -> TestResult<BTreeMap<Slots, usize>> {
    let mut sites = BTreeMap::new();
    for (index, site) in sources.sites.iter().enumerate() {
        let sub = site
            .sub
            .map(|value| -> TestResult<u64> { Ok(u64::try_from(locations.ordinal(value)?)?) })
            .transpose()?;
        let slots = Slots {
            handler: u64::try_from(locations.ordinal(site.handler)?)?,
            contract: u64::try_from(locations.ordinal(site.contract)?)?,
            sub,
        };
        if sites.insert(slots, index).is_some() {
            return Err("two original rows share the same physical occurrence".into());
        }
    }
    Ok(sites)
}
