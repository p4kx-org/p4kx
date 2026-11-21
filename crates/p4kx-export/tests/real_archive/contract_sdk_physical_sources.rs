// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{BTreeMap, BTreeSet};

use p4kx_extraction::{
    ArchiveEntrySource, FieldOrigin, InputScope, PhysicalSourceKind, PhysicalSourceRef,
    SourceEncryptionState, SourceInventory, SourceLocation, SourceUseDescription, SourceUseRef,
    SourceUseRole,
};

use super::TestResult;

#[path = "contract_sdk_physical_facts.rs"]
mod facts;

#[path = "contract_sdk_physical_cells.rs"]
mod cells;

#[cfg(test)]
#[path = "contract_sdk_physical_sources_tests.rs"]
mod tests;

type Uses<'a> = BTreeMap<SourceUseRef, &'a SourceUseDescription>;
type Mapping<'expected, 'actual> =
    BTreeMap<&'expected PhysicalSourceRef, &'actual PhysicalSourceRef>;

pub(super) struct Comparison<'expected, 'actual> {
    mapping: Mapping<'expected, 'actual>,
    expected_uses: Uses<'expected>,
    actual_uses: Uses<'actual>,
}

pub(super) fn original_source(
    expected: &SourceInventory,
    actual: &SourceInventory,
    original: &PhysicalSourceRef,
) -> TestResult<PhysicalSourceRef> {
    let comparison = Comparison::read(expected, actual)?;
    comparison.verify(expected, actual)?;
    Ok(comparison.mapped(original)?.clone())
}

impl<'expected, 'actual> Comparison<'expected, 'actual> {
    fn read(
        expected: &'expected SourceInventory,
        actual: &'actual SourceInventory,
    ) -> TestResult<Self> {
        let mut mapping = BTreeMap::new();
        let mut admitted = BTreeSet::new();
        for source in expected.iter_sources() {
            let mut matches = actual
                .iter_sources()
                .filter(|actual| facts::same(source, actual));
            let target = matches
                .next()
                .ok_or("SDK inventory lost original physical Entry facts")?;
            if matches.next().is_some() || !admitted.insert(target.id()) {
                return Err(
                    "original physical Entry facts have no unique one-to-one SDK source match"
                        .into(),
                );
            }
            mapping.insert(source.id(), target.id());
        }
        Ok(Self {
            mapping,
            expected_uses: metadata_uses(expected),
            actual_uses: metadata_uses(actual),
        })
    }

    fn mapped(&self, original: &PhysicalSourceRef) -> TestResult<&'actual PhysicalSourceRef> {
        self.mapping.get(original).copied().ok_or_else(|| {
            "an original physical ancestor or metadata contributor is not mapped".into()
        })
    }

    fn verify(&self, expected: &SourceInventory, actual: &SourceInventory) -> TestResult {
        for source in expected.iter_sources() {
            let target_id = self.mapped(source.id())?;
            let target = actual
                .iter_sources()
                .find(|target| target.id() == target_id)
                .ok_or("mapped SDK physical source is absent")?;
            if source.ancestors().len() != target.ancestors().len() {
                return Err("physical ancestor counts differ".into());
            }
            for (expected, actual) in source.ancestors().iter().zip(target.ancestors()) {
                if self.mapped(expected)? != actual {
                    return Err("original physical ancestor order or identity differs".into());
                }
            }
            let (
                PhysicalSourceKind::ArchiveEntry(expected),
                PhysicalSourceKind::ArchiveEntry(actual),
            ) = (source.kind(), target.kind())
            else {
                return Err(
                    "the original Archive Entry has unsupported or missing physical facts".into(),
                );
            };
            self.entry(expected, actual)?;
        }
        Ok(())
    }

    fn entry(&self, expected: &ArchiveEntrySource, actual: &ArchiveEntrySource) -> TestResult {
        self.origin(expected.path().origin(), actual.path().origin())?;
        self.origin(expected.zip_flags().origin(), actual.zip_flags().origin())?;
        cells::declarations(self, expected.declarations(), actual.declarations())
    }

    pub(super) fn origin(&self, expected: &FieldOrigin, actual: &FieldOrigin) -> TestResult {
        if expected.source_set_complete() != actual.source_set_complete()
            || expected.contains_encrypted_source() != actual.contains_encrypted_source()
        {
            return Err("original metadata completeness or encryption marker differs".into());
        }
        let expected = self.resolve(expected, true)?;
        let actual = self.resolve(actual, false)?;
        if expected.len() != actual.len() || expected.iter().any(|used| !actual.contains(used)) {
            return Err(
                "original metadata source, scope, role, coordinate or effective encryption differs"
                    .into(),
            );
        }
        Ok(())
    }

    fn resolve<'read>(
        &'read self,
        origin: &FieldOrigin,
        expected: bool,
    ) -> TestResult<Vec<MetadataUse<'read>>> {
        let inventory = if expected {
            &self.expected_uses
        } else {
            &self.actual_uses
        };
        let mut uses = Vec::new();
        for reference in origin.source_uses_refs() {
            let observation = inventory
                .get(reference)
                .ok_or("physical metadata has a dangling or non-metadata source-use reference")?;
            let SourceLocation::Metadata { field } = observation.location() else {
                return Err("physical metadata source coordinate is not a metadata field".into());
            };
            let source = if expected {
                self.mapped(observation.source())?
            } else {
                observation.source()
            };
            let value = MetadataUse {
                source,
                scope: observation.scope(),
                role: observation.role(),
                field,
                encryption: observation.effective_encryption(),
            };
            if !uses.contains(&value) {
                uses.push(value);
            }
        }
        Ok(uses)
    }
}

fn metadata_uses(inventory: &SourceInventory) -> Uses<'_> {
    inventory
        .iter_uses()
        .filter(|used| {
            matches!(
                used.scope(),
                InputScope::ArchiveDirectoryMetadata | InputScope::FileMetadata
            ) && matches!(used.location(), SourceLocation::Metadata { .. })
        })
        .map(|used| (used.id().clone(), used))
        .collect()
}

#[derive(PartialEq, Eq)]
struct MetadataUse<'a> {
    source: &'a PhysicalSourceRef,
    scope: InputScope,
    role: SourceUseRole,
    field: &'a str,
    encryption: SourceEncryptionState,
}
