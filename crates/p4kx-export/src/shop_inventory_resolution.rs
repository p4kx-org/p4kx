// SPDX-License-Identifier: MIT OR Apache-2.0
// SPDX-FileCopyrightText: 2026 the P4KX contributors

use std::collections::{HashMap, HashSet};

use p4kx_extraction::{SourceUse, SourceValue, SourcedValue};
use schemars::JsonSchema;
use serde::Serialize;

use crate::catalog_read::original::{AuthoredEntryPath, EntryCapture, PhysicalEntryRef};
use crate::contract::Shop;
use crate::schema_fields::every_field_required;
use crate::shop_containers::{Component, Containers, Inventory};
use crate::shop_inventories::{self, Inventories, ItemList};
#[path = "shop_inventory_candidate.rs"]
mod candidates;

use crate::shop_inventory_file::InventoryFileEvidence;
use crate::shop_prefabs::Prefabs;
use crate::shop_registry::RegistryShop;
use crate::{CatalogError, ExportAccountingError};
use candidates::{CapturedCandidates, InventoryCandidate};

/// One actual shop's inventory declaration and resolution.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(transform = every_field_required)]
pub(crate) struct ShopInventoryResolution {
    pub(crate) shop_key: SourcedValue<String>,
    status: SourcedValue<ResolutionStatus>,
    candidates: SourcedValue<Vec<SourcedValue<InventoryCandidate>>>,
    physical_targets: SourcedValue<Vec<SourcedValue<PhysicalEntryRef>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
#[schemars(extend("x-p4kx-vocabularyClass" = "control"))]
enum ResolutionStatus {
    Resolved,
    Placeholder,
    Ambiguous,
    Missing,
    Unreadable,
    Unscanned,
    NoDeclaredFile,
}

pub(crate) struct InventoryResolver<'read, 'input> {
    containers: &'read Containers,
    inventories: &'read Inventories,
    lists: &'read HashMap<&'read str, ItemList>,
    prefabs: &'read Prefabs,
    capture: &'read EntryCapture<'input>,
    declaration_complete: bool,
}

struct Decision<'read> {
    status: ResolutionStatus,
    files: TargetFiles<'read>,
    complete: bool,
}

#[derive(Clone, Copy)]
enum TargetFiles<'read> {
    None,
    Stem(&'read [InventoryFileEvidence]),
    Candidates,
}

#[derive(Default)]
struct Targets {
    references: Vec<SourcedValue<PhysicalEntryRef>>,
    uses: Vec<SourceUse>,
}

impl Targets {
    fn append(
        &mut self,
        files: &[InventoryFileEvidence],
        capture: &EntryCapture<'_>,
    ) -> Result<(), CatalogError> {
        self.references
            .try_reserve(files.len())
            .map_err(CatalogError::SourceUseAllocation)?;
        for file in files {
            let reference = file.reference(capture)?;
            let (uses, _) = reference.contributions();
            candidates::extend(&mut self.uses, uses)?;
            self.references
                .push(reference.into_cell(capture.source_uses()));
        }
        Ok(())
    }
}

impl<'read, 'input> InventoryResolver<'read, 'input> {
    pub(crate) const fn new(
        containers: &'read Containers,
        inventories: &'read Inventories,
        lists: &'read HashMap<&'read str, ItemList>,
        prefabs: &'read Prefabs,
        capture: &'read EntryCapture<'input>,
        declaration_complete: bool,
    ) -> Self {
        Self {
            containers,
            inventories,
            lists,
            prefabs,
            capture,
            declaration_complete,
        }
    }

    pub(crate) fn row(
        &self,
        registry: &RegistryShop,
        shop: &Shop,
    ) -> Result<ShopInventoryResolution, CatalogError> {
        let authored = registry
            .id
            .authored()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        let key = AuthoredEntryPath::derived(self.capture, &[authored], &shop.key)?
            .retain_in(self.capture)?;
        let sources = self.capture.source_uses();
        let observation = self.containers.inventory_observations(&shop.entity_guid);
        let mut captured =
            CapturedCandidates::read(observation, self.capture, self.declaration_complete)?;
        if let Some(rejected) = self.prefabs.rejected.get(&shop.entity_guid) {
            captured.rejected(rejected, self.capture)?;
        }
        let (key_uses, key_complete) = key.contributions();
        candidates::extend(&mut captured.uses, key_uses)?;
        let basis = sources.shared_origin(&captured.uses, captured.complete && key_complete);
        let decision = self.decide(
            &shop.entity_guid,
            observation.map(|(components, _)| components),
        );
        let Targets {
            references,
            mut uses,
        } = self.targets(decision.files, &captured.values)?;
        let from_candidates = matches!(decision.files, TargetFiles::Candidates);
        let target_complete =
            decision.complete && (!from_candidates || self.inventories.complete_directory());
        let physical_targets = sources.value_with_shared_origin(
            SourceValue::Present(references),
            &basis,
            &uses,
            target_complete,
        )?;
        if from_candidates {
            uses.clear();
        }
        decision.add_content(&mut uses)?;
        let status = sources.value_with_shared_origin(
            SourceValue::Present(decision.status),
            &basis,
            &uses,
            decision.complete,
        )?;
        Ok(ShopInventoryResolution {
            shop_key: candidates::text_cell(&key, sources)?,
            status,
            candidates: sources.value_with_shared_origin(
                SourceValue::Present(captured.values),
                &basis,
                &[],
                true,
            )?,
            physical_targets,
        })
    }

    fn targets(
        &self,
        files: TargetFiles<'_>,
        candidates: &[SourcedValue<InventoryCandidate>],
    ) -> Result<Targets, CatalogError> {
        let mut targets = Targets::default();
        match files {
            TargetFiles::None => {}
            TargetFiles::Stem(files) => targets.append(files, self.capture)?,
            TargetFiles::Candidates => {
                let mut seen = HashSet::new();
                for candidate in candidates {
                    let Some(InventoryCandidate::File { stem, .. }) = candidate.value() else {
                        continue;
                    };
                    let stem = stem
                        .value()
                        .ok_or(ExportAccountingError::ConflictingSourceDatum)?
                        .as_str();
                    if seen.contains(stem) {
                        continue;
                    }
                    seen.try_reserve(1)
                        .map_err(CatalogError::SourceUseAllocation)?;
                    seen.insert(stem);
                    targets.append(self.inventories.files(stem), self.capture)?;
                }
            }
        }
        Ok(targets)
    }

    fn decide(&self, entity: &str, components: Option<&[Component]>) -> Decision<'read> {
        let status = match self.containers.inventory(entity) {
            Inventory::One(stem) => return self.file(stem),
            Inventory::Ambiguous => ResolutionStatus::Ambiguous,
            Inventory::None
                if components.is_some_and(|parts| parts.contains(&Component::NoFile)) =>
            {
                ResolutionStatus::NoDeclaredFile
            }
            Inventory::Unknown | Inventory::None => ResolutionStatus::Unscanned,
        };
        Decision {
            status,
            files: if status == ResolutionStatus::Ambiguous {
                TargetFiles::Candidates
            } else {
                TargetFiles::None
            },
            complete: status != ResolutionStatus::Unscanned,
        }
    }

    fn file(&self, stem: &str) -> Decision<'read> {
        let files = self.inventories.files(stem);
        let directory_complete = self.inventories.complete_directory();
        let status = if files.len() > 1 {
            ResolutionStatus::Ambiguous
        } else if shop_inventories::is_placeholder(stem) {
            ResolutionStatus::Placeholder
        } else {
            match self.lists.get(stem) {
                Some(ItemList::Items { .. }) if !files.is_empty() => ResolutionStatus::Resolved,
                Some(ItemList::Unreadable(_)) => ResolutionStatus::Unreadable,
                Some(ItemList::Missing) if directory_complete => ResolutionStatus::Missing,
                _ => ResolutionStatus::Unscanned,
            }
        };
        let complete = directory_complete
            && !matches!(
                status,
                ResolutionStatus::Unscanned | ResolutionStatus::Unreadable
            );
        Decision {
            status,
            files: TargetFiles::Stem(files),
            complete,
        }
    }
}

impl Decision<'_> {
    fn add_content(&self, uses: &mut Vec<SourceUse>) -> Result<(), CatalogError> {
        if self.status != ResolutionStatus::Resolved {
            return Ok(());
        }
        let TargetFiles::Stem([file]) = self.files else {
            return Err(ExportAccountingError::ConflictingSourceDatum.into());
        };
        let content = file
            .content
            .as_ref()
            .ok_or(ExportAccountingError::UnboundHealthSubject)?;
        candidates::extend(uses, std::slice::from_ref(content))
    }
}
